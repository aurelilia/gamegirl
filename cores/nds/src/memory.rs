// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::ptr;

use arm_cpu::{interface::RwType, Cpu};
use common::{
    components::{
        debugger::Severity,
        memory::{MemoryMappedSystem, MemoryMapper},
    },
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};

use super::{Nds7, Nds9};
use crate::{addr::*, dma::Dmas, timer::Timers, CpuDevice, Nds, NdsCpu};

pub const KB: usize = 1024;
pub const MB: usize = KB * KB;

pub const BIOS7: &[u8] = include_bytes!("bios7.bin");
pub const BIOS9: &[u8] = include_bytes!("bios9.bin");

/// Memory struct containing the NDS's memory regions along with page tables
/// and other auxiliary cached information relating to memory.
/// A lot is separated by the 2 CPUs.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    pub psram: Box<[u8]>,
    wram: Box<[u8]>,

    wram7: Box<[u8]>,
    inst_tcm: Box<[u8]>,
    data_tcm: Box<[u8]>,

    pub mapper: CpuDevice<MemoryMapper<8192>>,
    wait_word: CpuDevice<[u16; 32]>,
    wait_other: CpuDevice<[u16; 32]>,
}

impl Nds {
    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {
        MemoryMapper::init_pages(&mut self.nds7());
        MemoryMapper::init_pages(&mut self.nds9());
    }

    pub fn try_get_mmio_shared<DS: NdsCpu>(ds: &DS, addr: u32) -> u16 {
        match addr & 0xFFFF {
            // Interrupt control
            IME => ds.cpur().ime as u16,
            IE_L => ds.cpur().ie.low(),
            IE_H => ds.cpur().ie.high(),
            IF_L => ds.cpur().if_.low(),
            IF_H => ds.cpur().if_.high(),

            _ => {
                ds.debugger.log(
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

            _ => ds.debugger.log(
                "unknown-io-write",
                format!("Write to unknown IO register {addr:08X}"),
                Severity::Warning,
            ),
        }
    }
}

impl Nds7 {
    pub fn get_slow<T: RwType>(&self, addr: u32) -> T {
        match addr {
            0x400_0000..=0x400_1010 if T::WIDTH == 1 && addr.is_bit(0) => {
                T::from_u8(self.get_mmio(addr).high())
            }
            0x400_0000..=0x400_1010 if T::WIDTH == 1 => T::from_u8(self.get_mmio(addr).low()),
            0x400_0000..=0x400_1010 if T::WIDTH == 2 => T::from_u16(self.get_mmio(addr)),
            0x400_0000..=0x400_1010 if T::WIDTH == 4 => {
                T::from_u32(word(self.get_mmio(addr), self.get_mmio(addr + 2)))
            }

            _ => T::from_u8(0),
        }
    }

    pub fn set_slow<T: RwType>(&mut self, addr: u32, value: T) {
        match addr {
            0x400_0000..=0x400_1010 if T::WIDTH == 1 && addr.is_bit(0) => {
                self.set_mmio(addr, hword(self.get_mmio(addr).low(), value.u8()))
            }
            0x400_0000..=0x400_1010 if T::WIDTH == 1 => {
                self.set_mmio(addr, hword(value.u8(), self.get_mmio(addr).high()))
            }
            0x400_0000..=0x400_1010 if T::WIDTH == 2 => self.set_mmio(addr, value.u16()),
            0x400_0000..=0x400_1010 if T::WIDTH == 4 => {
                self.set_mmio(addr, value.u16());
                self.set_mmio(addr, value.u32().high());
            }

            _ => (),
        }
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let addr = addr & !1;
        match addr {
            _ => Nds::try_get_mmio_shared(self, addr),
        }
    }

    fn set_mmio(&mut self, addr: u32, value: u16) {
        let addr = addr & !1;
        Nds::try_set_mmio_shared(self, addr, value);
    }
}

impl Nds9 {
    pub fn get_slow<T: RwType>(&self, addr: u32) -> T {
        match addr {
            0x400_0000..=0x400_0520 if T::WIDTH == 1 && addr.is_bit(0) => {
                T::from_u8(self.get_mmio(addr).high())
            }
            0x400_0000..=0x400_0520 if T::WIDTH == 1 => T::from_u8(self.get_mmio(addr).low()),
            0x400_0000..=0x400_0520 if T::WIDTH == 2 => T::from_u16(self.get_mmio(addr)),
            0x400_0000..=0x400_0520 if T::WIDTH == 4 => {
                T::from_u32(word(self.get_mmio(addr), self.get_mmio(addr + 2)))
            }

            0xFFFF_0000..=0xFFFF_FFFF => unsafe {
                let ptr = BIOS9.as_ptr().add(addr.us() % BIOS9.len());
                ptr.cast::<T>().read()
            },

            _ => T::from_u8(0),
        }
    }

    pub fn set_slow<T: RwType>(&mut self, addr: u32, value: T) {
        match addr {
            0x400_0000..=0x400_0520 if T::WIDTH == 1 && addr.is_bit(0) => {
                self.set_mmio(addr, hword(self.get_mmio(addr).low(), value.u8()))
            }
            0x400_0000..=0x400_0520 if T::WIDTH == 1 => {
                self.set_mmio(addr, hword(value.u8(), self.get_mmio(addr).high()))
            }
            0x400_0000..=0x400_0520 if T::WIDTH == 2 => self.set_mmio(addr, value.u16()),
            0x400_0000..=0x400_0520 if T::WIDTH == 4 => {
                self.set_mmio(addr, value.u16());
                self.set_mmio(addr, value.u32().high());
            }

            _ => (),
        }
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let addr = addr & 0x1FFE;
        match addr {
            _ => Nds::try_get_mmio_shared(self, addr),
        }
    }

    fn set_mmio(&mut self, addr: u32, value: u16) {
        let addr = addr & 0x1FFE;
        match addr {
            _ => Nds::try_set_mmio_shared(self, addr, value),
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            psram: Box::new([0; 4 * MB]),
            wram: Box::new([0; 32 * KB]),
            wram7: Box::new([0; 64 * KB]),
            inst_tcm: Box::new([0; 32 * KB]),
            data_tcm: Box::new([0; 16 * KB]),

            mapper: [MemoryMapper::default(), MemoryMapper::default()],
            wait_word: [[0; 32]; 2],
            wait_other: [[0; 32]; 2],
        }
    }
}

unsafe impl Send for Memory {}

impl MemoryMappedSystem<8192> for Nds7 {
    type Usize = u32;
    const ADDR_MASK: &'static [usize] = &[
        0x3FFF, // ARM7 BIOS
        0,      // Unmapped
        0x7FFF, // PSRAM
        0x7FFF, // WRAM/WRAM7
        0,      // MMIO
        0,      // Unmapped
        0x7FFF, // VRAM
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
    ];
    const PAGE_POW: usize = 15;
    const MASK_POW: usize = 24;

    fn get_mapper(&self) -> &MemoryMapper<8192> {
        &self.memory.mapper[0]
    }

    fn get_mapper_mut(&mut self) -> &mut MemoryMapper<8192> {
        &mut self.memory.mapper[0]
    }

    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8 {
        unsafe fn offs(reg: &[u8], offs: usize) -> *mut u8 {
            let ptr = reg.as_ptr() as *mut u8;
            ptr.add(offs % reg.len())
        }

        match a {
            0x0000_0000..=0x00FF_FFFF if R => offs(BIOS7, a),
            0x0200_0000..=0x02FF_FFFF => offs(&self.memory.psram, a - 0x200_0000),
            // TODO not quite right...
            0x0300_0000..=0x037F_FFFF => offs(&self.memory.wram, a - 0x300_0000),
            0x0380_0000..=0x03FF_FFFF => offs(&self.memory.wram7, a - 0x380_0000),
            0x0600_0000..=0x06FF_FFFF if false => todo!(),

            _ => ptr::null::<u8>() as *mut u8,
        }
    }
}

impl MemoryMappedSystem<8192> for Nds9 {
    type Usize = u32;
    const ADDR_MASK: &'static [usize] = &[
        0x7FFF, // Instruction TCM
        0,      // Unmapped
        0x7FFF, // PSRAM
        0x7FFF, // WRAM
        0,      // MMIO
        0x7FFF, // Palette
        0x7FFF, // VRAM
        0x7FFF, // OAM
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
        0,      // Unmapped
    ];
    const PAGE_POW: usize = 15;
    const MASK_POW: usize = 24;

    fn get_mapper(&self) -> &MemoryMapper<8192> {
        &self.memory.mapper[1]
    }

    fn get_mapper_mut(&mut self) -> &mut MemoryMapper<8192> {
        &mut self.memory.mapper[1]
    }

    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8 {
        unsafe fn offs(reg: &[u8], offs: usize) -> *mut u8 {
            let ptr = reg.as_ptr() as *mut u8;
            ptr.add(offs % reg.len())
        }

        match a {
            0x0000_0000..=0x01FF_FFFF if R => offs(&self.memory.inst_tcm, a),
            0x0200_0000..=0x02FF_FFFF => offs(&self.memory.psram, a - 0x200_0000),
            0x0300_0000..=0x03FF_FFFF => offs(&self.memory.wram, a - 0x300_0000),

            // 0x0500_0000..=0x05FF_FFFF if (a & 0x1FFF) < 0x1000 => {
            //     offs(&self.ppu_a_nomut().palette, a - 0x500_0000)
            // }
            // 0x0500_0000..=0x05FF_FFFF => offs(&self.ppu_b_nomut().palette, a - 0x501_0000),
            // 0x0600_0000..=0x061F_FFFF => offs(&self.ppu_a_nomut().vram, a - 0x600_0000),
            // 0x0620_0000..=0x063F_FFFF => offs(&self.ppu_b_nomut().vram, a - 0x620_0000),
            // // TODO not quite right...
            // 0x0640_0000..=0x065F_FFFF => offs(&self.ppu_a_nomut().vram, a - 0x640_0000),
            // 0x0660_0000..=0x067F_FFFF => offs(&self.ppu_b_nomut().vram, a - 0x660_0000),
            // 0x0700_0000..=0x07FF_FFFF if (a & 0x1FFF) < 0x1000 => {
            //     offs(&self.ppu_a_nomut().oam, a - 0x700_0000)
            // }
            // 0x0700_0000..=0x07FF_FFFF => offs(&self.ppu_b_nomut().oam, a - 0x701_0000),
            0x0600_0000..=0x06FF_FFFF if false => todo!(),

            _ => ptr::null::<u8>() as *mut u8,
        }
    }
}
