// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{mem, ptr};

use arm_cpu::{
    interface::{ArmSystem, RwType},
    Cpu, Interrupt,
};
use common::{
    common::debugger::Severity,
    components::{
        memory_mapper::{MemoryMappedSystem, MemoryMapper},
        thin_pager::{ThinPager, RO, RW},
    },
    numutil::{get_u64, hword, set_u64, word, ByteArrayExt, NumExt, U16Ext, U32Ext},
};

use super::{Nds7, Nds9};
use crate::{
    addr::*,
    cpu::cp15::TcmState,
    graphics::vram::*,
    hw::{
        bios::{FREEBIOS7, FREEBIOS9},
        dma::Dmas,
        timer::Timers,
    },
    CpuDevice, Nds, NdsCpu,
};

pub const KB: usize = 1024;
pub const MB: usize = KB * KB;

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum WramStatus {
    All9 = 0,
    First7 = 1,
    First9 = 2,
    All7 = 3,
}

/// Memory struct containing the NDS's memory regions along with page tables
/// and other auxiliary cached information relating to memory.
/// A lot is separated by the 2 CPUs.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    pub psram: Box<[u8]>,
    wram: Box<[u8]>,
    pub wram_status: WramStatus,
    pub(super) postflg: bool,

    pub bios7: Box<[u8]>,
    pub bios9: Box<[u8]>,

    wram7: Box<[u8]>,
    pub(crate) tcm: [Box<[u8]>; 2],

    wait_word: CpuDevice<[u16; 32]>,
    wait_other: CpuDevice<[u16; 32]>,
    pub(crate) pager7: ThinPager,
    pub(crate) pager9: ThinPager,
}

impl Nds {
    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {
        // Init 7
        let p7 = &mut self.memory.pager7;
        p7.init(0xFFF_FFFF);
        p7.map(&self.memory.bios7, 0x000_0000..0x100_0000, RO);
        p7.map(&self.memory.psram, 0x200_0000..0x300_0000, RW);
        p7.map(&self.memory.wram7, 0x380_0000..0x400_0000, RW);

        // Init 9
        let p9 = &mut self.memory.pager9;
        p9.init(0xFFF_FFFF);
        p9.map(&self.memory.tcm[1], 0x000_0000..0x200_0000, RW);
        p9.map(&self.memory.psram, 0x200_0000..0x300_0000, RW);

        // Init V/WRAM
        self.gpu.vram.init_mappings(p7, p9);
        self.update_wram();
    }

    /// Evict and recreate WRAM mappings.
    pub(super) fn update_wram(&mut self) {
        self.memory.pager7.evict(0x300_0000..0x380_0000);
        self.memory.pager9.evict(0x300_0000..0x400_0000);
        match self.memory.wram_status {
            WramStatus::All7 => {
                self.memory
                    .pager7
                    .map(&self.memory.wram, 0x300_0000..0x380_0000, RW)
            }
            WramStatus::First9 => {
                self.memory
                    .pager9
                    .map(&self.memory.wram[..(16 * KB)], 0x300_0000..0x400_0000, RW);
                self.memory
                    .pager7
                    .map(&self.memory.wram[(16 * KB)..], 0x300_0000..0x380_0000, RW);
            }
            WramStatus::First7 => {
                self.memory
                    .pager7
                    .map(&self.memory.wram[..(16 * KB)], 0x300_0000..0x380_0000, RW);
                self.memory
                    .pager9
                    .map(&self.memory.wram[(16 * KB)..], 0x300_0000..0x400_0000, RW);
            }
            WramStatus::All9 => {
                self.memory
                    .pager9
                    .map(&self.memory.wram, 0x300_0000..0x400_0000, RW);
                // When the shared WRAM isn't mapped, the ARM7 WRAM takes over
                self.memory
                    .pager7
                    .map(&self.memory.wram7, 0x300_0000..0x380_0000, RW)
            }
        }
    }

    pub(super) fn maybe_irq_to_other(&mut self, cpu: usize, intr: Option<Interrupt>) {
        if let Some(intr) = intr {
            self.send_irq(cpu ^ 1, intr);
        }
    }

    pub fn send_irq(&mut self, cpu: usize, irq: Interrupt) {
        if cpu == 0 {
            Cpu::request_interrupt(&mut self.nds7(), irq);
        } else {
            Cpu::request_interrupt(&mut self.nds9(), irq);
        }
    }
}

impl Nds7 {
    pub fn get<T: RwType>(&mut self, addr_unaligned: u32) -> T {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        if addr > 0xFFF_FFFF {
            return T::from_u32(0);
        }
        if let Some(read) = self.memory.pager7.read(addr) {
            return read;
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // MMIO
            0x04 => self.get_mmio(addr),
            _ => T::from_u8(0),
        }
    }

    pub fn set<T: RwType>(&mut self, addr: u32, value: T) {
        if addr > 0xFFF_FFFF {
            return;
        }
        if let Some(write) = self.memory.pager7.write(addr) {
            *write = value;
            return;
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // MMIO
            0x04 => self.set_mmio(addr, value),
            _ => {
                self.debugger().running = false;
                log::info!("Invalid write: {addr:X}");
            }
        }
    }
}

impl Nds9 {
    pub fn get<T: RwType>(&mut self, addr_unaligned: u32) -> T {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        if addr <= 0xFFF_FFFF {
            for tcm in 0..2 {
                if self.cp15.tcm_state[tcm] == TcmState::Rw
                    && self.cp15.tcm_range[tcm].contains(&addr)
                {
                    return self.memory.tcm[tcm]
                        .get_wrap(addr.us() - self.cp15.tcm_range[tcm].start.us());
                }
            }

            if let Some(read) = self.memory.pager9.read(addr) {
                return read;
            }
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // Basic
            0xFF if (0xFFFF_0000..0xFFFF_1000).contains(&addr) => {
                self.memory.bios9.get_exact(a & 0xFFFF)
            }

            // PPU
            0x05 => self.gpu.ppus[a.bit(10)].palette.get_wrap(a),
            0x07 => self.gpu.ppus[a.bit(10)].oam.get_wrap(a),

            // MMIO
            0x04 => self.get_mmio(addr),

            _ => {
                log::info!("Invalid read: {addr:X}");
                T::from_u32(0)
            }
        }
    }

    pub fn set<T: RwType>(&mut self, addr: u32, value: T) {
        for tcm in 0..2 {
            if self.cp15.tcm_state[tcm] != TcmState::None
                && self.cp15.tcm_range[tcm].contains(&addr)
            {
                return self.memory.tcm[tcm]
                    .set_wrap(addr.us() - self.cp15.tcm_range[tcm].start.us(), value);
            }
        }
        if addr > 0xFFF_FFFF {
            return;
        }
        if let Some(write) = self.memory.pager9.write(addr) {
            *write = value;
            return;
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // PPU
            0x05 => self.gpu.ppus[a.bit(10)].palette.set_wrap(a, value),
            0x07 => self.gpu.ppus[a.bit(10)].oam.set_wrap(a, value),

            // MMIO
            0x04 => self.set_mmio(addr, value),

            _ => {
                log::info!("Invalid write: {addr:X}");
            }
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            psram: Box::new([0; 4 * MB]),
            wram: Box::new([0; 32 * KB]),
            wram_status: WramStatus::All7,
            postflg: false,
            bios7: FREEBIOS7.into(),
            bios9: FREEBIOS9.into(),

            wram7: Box::new([0; 64 * KB]),
            tcm: [Box::new([0; 16 * KB]), Box::new([0; 32 * KB])],

            wait_word: [[0; 32]; 2],
            wait_other: [[0; 32]; 2],
            pager7: ThinPager::default(),
            pager9: ThinPager::default(),
        }
    }
}

unsafe impl Send for Memory {}
