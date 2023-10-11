// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::{Index, IndexMut};

use common::numutil::{hword, word, NumExt, U16Ext, U32Ext};

use crate::{
    addr::{DMABASE, DMACTRL, DMAINT, MMIOBASE},
    dma::Dma,
    PlayStation,
};

const KB: usize = 1024;
const MB: usize = KB * KB;
const BIOS: &[u8] = include_bytes!("bios.bin");

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    ram: [u8; 2 * MB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    scratchpad: [u8; KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub mmio: [u32; 8 * KB / 4],
}

impl PlayStation {
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        match Self::phys_addr(addr) {
            0x0000_0000..=0x001F_FFFF => self.memory.ram[addr.us() - 0xA000_0000],
            0x1F80_0000..=0x1F80_03FF => self.memory.scratchpad[addr.us() - 0x1F80_0000],

            0x1FC0_0000..=0x1FC7_FFFF => BIOS[addr.us() - 0xBFC0_0000],
            unknown => {
                log::warn!(
                    "Read from unmapped address 0x{addr:X} (physical address 0x{unknown:X}), reading 0xFF"
                );
                0xFF
            }
        }
    }

    pub fn read_hword(&mut self, addr: u32) -> u16 {
        hword(self.read_byte(addr), self.read_byte(addr + 1))
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        match Self::phys_addr(addr) {
            0x1F80_1000..=0x1F80_1FFF => self.memory.mmio[(addr.us() - 0x1F80_1000) / 4],
            _ => word(self.read_hword(addr), self.read_hword(addr + 2)),
        }
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        match Self::phys_addr(addr) {
            0x0000_0000..=0x001F_FFFF => self.memory.ram[addr.us() - 0xA000_0000] = value,
            0x1F80_0000..=0x1F80_03FF => self.memory.scratchpad[addr.us() - 0x1F80_0000] = value,
            0x1f801800 => panic!("HA"),

            unknown => log::warn!(
                "Write to unmapped address 0x{addr:X} (physical address 0x{unknown:X}), discarding write"
            ),
        }
    }

    pub fn write_hword(&mut self, addr: u32, value: u16) {
        self.write_byte(addr, value.low());
        self.write_byte(addr + 1, value.high());
    }

    pub fn write_word(&mut self, addr: u32, value: u32) {
        match Self::phys_addr(addr) {
            0x1F80_1000..=0x1F80_1FFF => self[addr - 0x1F80_1000] = value,
            _ => {
                self.write_hword(addr, value.low());
                self.write_hword(addr + 2, value.high());
            }
        }
    }

    pub fn set_iow(&mut self, addr: u32, value: u32) {
        match addr - MMIOBASE {
            // DMA
            DMAINT => self[DMAINT] = value & 0xFFFF_803F,
            // Address register. Upper bits unused
            _ if (addr > DMABASE && addr < DMACTRL) && addr & 0xF == 0 => {
                self[addr] = value & 0xFF_FFFF
            }
            // Channel control register.
            _ if (addr > DMABASE && addr < DMACTRL) && addr & 0xF == 8 => {
                self[addr] = value; // TODO some bits are supposed to be always
                                    // 0...
                Dma::maybe_trigger(self, addr);
            }

            _ => self[addr] = value,
        }
    }

    fn phys_addr(addr: u32) -> u32 {
        const MASKS: [u32; 8] = [
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0x7FFF_FFFF,
            0x1FFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
        ];
        let mask = MASKS[addr.bits(29, 3).us()];
        addr & mask
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            ram: [0; 2 * MB],
            scratchpad: [0; KB],
            mmio: [0; 2 * KB],
        }
    }
}

impl Index<u32> for PlayStation {
    type Output = u32;

    fn index(&self, addr: u32) -> &Self::Output {
        assert!(addr < 0x1FFF);
        &self.memory.mmio[(addr / 4).us()]
    }
}

impl IndexMut<u32> for PlayStation {
    fn index_mut(&mut self, addr: u32) -> &mut Self::Output {
        assert!(addr < 0x1FFF);
        &mut self.memory.mmio[(addr / 4).us()]
    }
}
