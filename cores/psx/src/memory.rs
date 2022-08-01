// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::numutil::{hword, word, NumExt, U16Ext, U32Ext};

use crate::PlayStation;

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
    pub mmio: [u8; 8 * KB],
}

impl PlayStation {
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        match Self::phys_addr(addr) {
            0x0000_0000..=0x001F_FFFF => self.memory.ram[addr.us() - 0xA000_0000],
            0x1F80_0000..=0x1F80_03FF => self.memory.scratchpad[addr.us() - 0x1F80_0000],
            0x1F80_1000..=0x1F80_1FFF => self.memory.mmio[addr.us() - 0x1F80_1000],

            0x1FC0_0000..=0x1FC7_FFFF => BIOS[addr.us() - 0xBFC0_0000],
            unknown => {
                log::warn!(
                    "Read from unmapped address {addr} (physical address {unknown}), reading 0xFF"
                );
                0xFF
            }
        }
    }

    pub fn read_hword(&mut self, addr: u32) -> u16 {
        hword(self.read_byte(addr), self.read_byte(addr + 1))
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        word(self.read_hword(addr), self.read_hword(addr + 2))
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        match Self::phys_addr(addr) {
            0x0000_0000..=0x001F_FFFF => self.memory.ram[addr.us() - 0xA000_0000] = value,
            0x1F80_0000..=0x1F80_03FF => self.memory.scratchpad[addr.us() - 0x1F80_0000] = value,
            0x1F80_1000..=0x1F80_1FFF => self.memory.mmio[addr.us() - 0x1F80_1000] = value,

            unknown => log::warn!(
                "Write to unmapped address {addr} (physical address {unknown}), discarding write"
            ),
        }
    }

    pub fn write_hword(&mut self, addr: u32, value: u16) {
        self.write_byte(addr, value.low());
        self.write_byte(addr + 1, value.high());
    }

    pub fn write_word(&mut self, addr: u32, value: u32) {
        self.write_hword(addr, value.low());
        self.write_hword(addr + 2, value.high());
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
            mmio: [0; 8 * KB],
        }
    }
}
