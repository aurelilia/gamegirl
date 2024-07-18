// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{mem, ptr};

use arm_cpu::{interface::RwType, Cpu};
use common::{
    common::debugger::Severity,
    components::memory_mapper::{MemoryMappedSystem, MemoryMapper},
    numutil::{get_u64, hword, set_u64, word, ByteArrayExt, NumExt, U16Ext, U32Ext},
};

use super::{Nds7, Nds9};
use crate::{addr::*, dma::Dmas, graphics::vram::*, timer::Timers, CpuDevice, Nds, NdsCpu};

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

    pub bios7: Box<[u8]>,
    pub bios9: Box<[u8]>,

    wram7: Box<[u8]>,
    inst_tcm: Box<[u8]>,
    data_tcm: Box<[u8]>,

    wait_word: CpuDevice<[u16; 32]>,
    wait_other: CpuDevice<[u16; 32]>,
}

impl Nds {
    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {}

    pub fn try_get_mmio_shared<DS: NdsCpu>(ds: &DS, addr: u32) -> u16 {
        // TODO Timer and DMA reads...
        match addr & 0xFFFF {
            // Interrupt control
            IME => ds.cpur().ime as u16,
            IE_L => ds.cpur().ie.low(),
            IE_H => ds.cpur().ie.high(),
            IF_L => ds.cpur().if_.low(),
            IF_H => ds.cpur().if_.high(),

            // GPU
            DISPSTAT => ds.gpu.dispstat[DS::I].into(),
            VCOUNT => ds.gpu.vcount,

            // Input
            KEYCNT => ds.input.cnt[DS::I].into(),
            KEYINPUT => ds.keyinput(),

            _ => {
                ds.c.debugger.log(
                    "unknown-io-read",
                    format!("Read from unknown IO register {addr:08X}"),
                    Severity::Warning,
                );
                0
            }
        }
    }

    pub fn try_set_mmio_shared<DS: NdsCpu>(dsx: &mut DS, addr: u32, value: u16) {
        let ds = dsx.deref_mut();
        match addr & 0xFFFF {
            // Interrupts
            IME => {
                dsx.cpu().ime = value.is_bit(0);
                Cpu::check_if_interrupt(dsx);
            }
            IE_L => {
                dsx.cpu().ie = word(value, dsx.cpu().ie.high());
                Cpu::check_if_interrupt(dsx);
            }
            IE_H => {
                dsx.cpu().ie = word(dsx.cpu().ie.low(), value);
                Cpu::check_if_interrupt(dsx);
            }
            IF_L => dsx.cpu().if_ &= (!value).u32() | 0xFFFF_0000,
            IF_H => dsx.cpu().if_ &= ((!value).u32() << 16) | 0x0000_FFFF,

            // Timers
            TM0CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 0, value),
            TM1CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 1, value),
            TM2CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 2, value),
            TM3CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 3, value),

            // DMAs
            0xBA => Dmas::ctrl_write(dsx, 0, value),
            0xC6 => Dmas::ctrl_write(dsx, 1, value),
            0xD2 => Dmas::ctrl_write(dsx, 2, value),
            0xDE => Dmas::ctrl_write(dsx, 3, value),

            // Shared GPU stuff
            DISPSTAT => {
                let disp: u16 = ds.gpu.dispstat[DS::I].into();
                ds.gpu.dispstat[DS::I] = ((disp & 0b111) | (value & !0b1100_0111)).into();
            }

            // Input
            KEYCNT => ds.input.cnt[DS::I] = value.into(),

            _ => ds.c.debugger.log(
                "unknown-io-write",
                format!("Write to unknown IO register {addr:08X}"),
                Severity::Warning,
            ),
        }
    }
}

impl Nds7 {
    pub fn get<T: RwType>(&self, addr_unaligned: u32) -> T {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        let region = addr >> 24;
        let a = addr.us();
        match addr {
            // Basic
            0x00 => self.memory.bios7.get_wrap(a),
            0x02 => self.memory.psram.get_wrap(a),

            // WRAM
            0x03 if a > 0x380_0000 => self.memory.wram7.get_wrap(a),
            0x03 => match self.memory.wram_status {
                WramStatus::All7 => self.memory.wram.get_wrap(a),
                WramStatus::First9 => self.memory.wram[(16 * KB)..].get_wrap(a),
                WramStatus::First7 => self.memory.wram[..(16 * KB)].get_wrap(a),
                WramStatus::All9 => T::from_u32(0),
            },

            // MMIO
            0x04 => match T::WIDTH {
                1 if addr.is_bit(0) => T::from_u8(self.get_mmio(addr).high()),
                1 => T::from_u8(self.get_mmio(addr).low()),
                2 => T::from_u16(self.get_mmio(addr)),
                4 => T::from_u32(word(self.get_mmio(addr), self.get_mmio(addr + 2))),
                _ => unreachable!(),
            },

            // VRAM-as-WRAM
            0x06 if let Some(ram) = self.gpu.vram.get7(a) => ram.get_wrap(a & 0x3_FFFF),

            _ => T::from_u8(0),
        }
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let addr = addr & !1;
        match addr {
            VRAMSTAT => hword(self.gpu.vram.vram_stat(), self.memory.wram_status as u8),
            EXTKEYIN => self.keyinput_ext(),
            _ => Nds::try_get_mmio_shared(self, addr),
        }
    }

    pub fn set<T: RwType>(&mut self, addr: u32, value: T) {
        let region = addr >> 24;
        let a = addr.us();
        match region {
            // Basic
            0x02 => self.memory.psram.set_wrap(a, value), // does this wrap actually

            // WRAM
            0x03 if a > 0x380_0000 => self.memory.wram7.set_wrap(a, value),
            0x03 => match self.memory.wram_status {
                WramStatus::All7 => self.memory.wram.set_wrap(a, value),
                WramStatus::First9 => self.memory.wram[(16 * KB)..].set_wrap(a, value),
                WramStatus::First7 => self.memory.wram[..(16 * KB)].set_wrap(a, value),
                WramStatus::All9 => (),
            },

            // MMIO
            0x04 => match T::WIDTH {
                1 if addr.is_bit(0) => {
                    self.set_mmio(addr, hword(self.get_mmio(addr).low(), value.u8()))
                }
                1 => self.set_mmio(addr, hword(value.u8(), self.get_mmio(addr).high())),
                2 => self.set_mmio(addr, value.u16()),
                4 => {
                    self.set_mmio(addr, value.u16());
                    self.set_mmio(addr + 2, value.u32().high());
                }
                _ => unreachable!(),
            },

            // VRAM-as-WRAM
            0x06 if let Some(ram) = self.gpu.vram.get7_mut(a) => ram.set_wrap(a & 0x3_FFFF, value),

            _ => (),
        }
    }

    fn set_mmio(&mut self, addr: u32, value: u16) {
        let addr = addr & !1;
        Nds::try_set_mmio_shared(self, addr, value);
    }
}

impl Nds9 {
    pub fn get<T: RwType>(&self, addr_unaligned: u32) -> T {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        let region = addr >> 24;
        let a = addr.us();
        match region {
            // Basic
            0x00 | 0x01 => self.memory.inst_tcm.get_wrap(a), // TODO accurate wrap
            0x02 => self.memory.psram.get_wrap(a),
            0xFF if addr >= 0xFFFF_0000 => self.memory.bios9.get_exact(a & 0xFFFF),

            // WRAM
            0x03 => match self.memory.wram_status {
                WramStatus::All9 => self.memory.wram.get_wrap(a),
                WramStatus::First7 => self.memory.wram[(16 * KB)..].get_wrap(a),
                WramStatus::First9 => self.memory.wram[..(16 * KB)].get_wrap(a),
                WramStatus::All7 => T::from_u32(0),
            },

            // PPU
            // TODO verify the bit is right
            0x05 => self.gpu.ppus[a.bit(12)].palette.get_wrap(a),
            0x07 => self.gpu.ppus[a.bit(12)].oam.get_wrap(a),

            // MMIO
            0x04 => match T::WIDTH {
                1 if addr.is_bit(0) => T::from_u8(self.get_mmio(addr).high()),
                1 => T::from_u8(self.get_mmio(addr).low()),
                2 => T::from_u16(self.get_mmio(addr)),
                4 => T::from_u32(word(self.get_mmio(addr), self.get_mmio(addr + 2))),
                _ => unreachable!(),
            },

            // DTCM
            _ if region == self.cp15.dtcm_region() => self.memory.data_tcm.get_wrap(a),

            _ => T::from_u32(0),
        }
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let addr = addr & 0x1FFE;
        match addr {
            // PPUs
            DISPCNT_L | DISPCNT_H | 0x08..0x60
                if let Some(val) = self.gpu.ppus[0].regs.read_mmio(addr) =>
            {
                val
            }
            0x1000 | 0x1002 | 0x1008..0x1060
                if let Some(val) = self.gpu.ppus[1].regs.read_mmio(addr & 0xFF) =>
            {
                val
            }

            // Graphics
            DISP3DCNT => self.gpu.gpu.cnt.into(),
            DISPCAPCNT_L => u32::from(self.gpu.capture.cnt).low(),
            DISPCAPCNT_H => u32::from(self.gpu.capture.cnt).high(),

            // RAM control
            VRAMCNT_A => hword(self.gpu.vram.ctrls[A].into(), self.gpu.vram.ctrls[B].into()),
            VRAMCNT_C => hword(self.gpu.vram.ctrls[C].into(), self.gpu.vram.ctrls[D].into()),
            VRAMCNT_E => hword(self.gpu.vram.ctrls[E].into(), self.gpu.vram.ctrls[F].into()),
            VRAMCNT_G => hword(self.gpu.vram.ctrls[G].into(), self.memory.wram_status as u8),
            VRAMCNT_H => hword(self.gpu.vram.ctrls[H].into(), self.gpu.vram.ctrls[I].into()),

            // Math
            DIVCNT_L => self.div.ctrl.into(),
            DIVCNT_H => 0,
            DIV_NUMER..DIV_DENOM => get_u64(self.div.numer, addr & 6),
            DIV_DENOM..DIV_RESULT => get_u64(self.div.denom, addr & 6),
            DIV_RESULT..DIV_REM => get_u64(self.div.result, addr & 6),
            DIV_REM..SQRTCNT_L => get_u64(self.div.rem, addr & 6),
            SQRTCNT_L => self.sqrt.ctrl.into(),
            SQRTCNT_H => 0,
            SQRT_RESULT_L => self.sqrt.result.low(),
            SQRT_RESULT_H => self.sqrt.result.high(),
            SQRT_INPUT..0x2C0 => get_u64(self.sqrt.input, addr & 6),

            _ => Nds::try_get_mmio_shared(self, addr),
        }
    }

    pub fn set<T: RwType>(&mut self, addr: u32, value: T) {
        let region = addr >> 24;
        let a = addr.us();
        match region {
            // Basic
            0x00 | 0x01 => self.memory.inst_tcm.set_wrap(a, value), // TODO accurate wrap
            0x02 => self.memory.psram.set_wrap(a, value),

            // WRAM
            0x03 => match self.memory.wram_status {
                WramStatus::All9 => self.memory.wram.set_wrap(a, value),
                WramStatus::First7 => self.memory.wram[(16 * KB)..].set_wrap(a, value),
                WramStatus::First9 => self.memory.wram[..(16 * KB)].set_wrap(a, value),
                WramStatus::All7 => (),
            },

            // PPU
            // TODO verify the bit is right
            0x05 => self.gpu.ppus[a.bit(12)].palette.set_wrap(a, value),
            0x07 => self.gpu.ppus[a.bit(12)].oam.set_wrap(a, value),

            // MMIO
            0x04 => match T::WIDTH {
                1 if addr.is_bit(0) => {
                    self.set_mmio_hword(addr, hword(self.get_mmio(addr).low(), value.u8()))
                }
                1 => self.set_mmio_hword(addr, hword(value.u8(), self.get_mmio(addr).high())),
                2 => self.set_mmio_hword(addr, value.u16()),
                4 => self.set_mmio_word(addr, value.u32()),
                _ => unreachable!(),
            },

            // DTCM
            _ if region == self.cp15.dtcm_region() => self.memory.data_tcm.set_wrap(a, value),

            _ => (),
        }
    }

    fn set_mmio_hword(&mut self, addr: u32, value: u16) {
        let addr = addr & 0x1FFE;
        match addr {
            // PPUs
            // TODO handle byte writes right
            DISPCNT_L | DISPCNT_H | 0x08..0x60 => self.gpu.ppus[0].regs.write_mmio(addr, value),
            0x1000 | 0x1002 | 0x1008..0x1060 => {
                self.gpu.ppus[1].regs.write_mmio(addr & 0xFF, value)
            }

            // Graphics
            DISP3DCNT => self.gpu.gpu.cnt = value.into(),
            DISPCAPCNT_L => {
                self.gpu.capture.cnt = word(value, u32::from(self.gpu.capture.cnt).high()).into()
            }
            DISPCAPCNT_H => {
                self.gpu.capture.cnt = word(u32::from(self.gpu.capture.cnt).low(), value).into()
            }

            // RAM control
            VRAMCNT_A => {
                self.gpu.vram.update_ctrl(A, value.low());
                self.gpu.vram.update_ctrl(B, value.high());
            }
            VRAMCNT_C => {
                self.gpu.vram.update_ctrl(C, value.low());
                self.gpu.vram.update_ctrl(D, value.high());
            }
            VRAMCNT_E => {
                self.gpu.vram.update_ctrl(E, value.low());
                self.gpu.vram.update_ctrl(F, value.high());
            }
            VRAMCNT_G => {
                self.gpu.vram.update_ctrl(G, value.low());
                self.memory.wram_status = unsafe { mem::transmute(value.high() & 3) };
            }
            VRAMCNT_H => {
                self.gpu.vram.update_ctrl(H, value.low());
                self.gpu.vram.update_ctrl(I, value.high());
            }

            // Math
            // TODO React to writes.
            DIVCNT_L => self.div.ctrl = value.into(),
            DIV_NUMER..DIV_DENOM => self.div.numer = set_u64(self.div.numer, addr & 6, value),
            DIV_DENOM..DIV_RESULT => self.div.denom = set_u64(self.div.denom, addr & 6, value),
            SQRTCNT_L => self.sqrt.ctrl = value.into(),
            SQRT_INPUT..0x2C0 => self.sqrt.input = set_u64(self.sqrt.input, addr & 6, value),

            _ => Nds::try_set_mmio_shared(self, addr, value),
        }
    }

    fn set_mmio_word(&mut self, addr: u32, value: u32) {
        let addr = addr & 0x1FFE;
        match addr {
            _ => {
                self.set_mmio_hword(addr, value.u16());
                self.set_mmio_hword(addr, value.u32().high());
            }
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            psram: Box::new([0; 4 * MB]),
            wram: Box::new([0; 32 * KB]),
            wram_status: WramStatus::All9,
            bios7: Box::new([]),
            bios9: Box::new([]),

            wram7: Box::new([0; 64 * KB]),
            inst_tcm: Box::new([0; 32 * KB]),
            data_tcm: Box::new([0; 16 * KB]),

            wait_word: [[0; 32]; 2],
            wait_other: [[0; 32]; 2],
        }
    }
}

unsafe impl Send for Memory {}
