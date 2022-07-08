// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    numutil::{hword, word, NumExt, U32Ext},
    psx::PlayStation,
};

const BIOS: &[u8] = include_bytes!("bios.bin");

impl PlayStation {
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0xBFC0_0000..=0xBFC7_FFFF => BIOS[addr.us() - 0xBFC0_0000],
            _ => 0xFF,
        }
    }

    pub fn read_hword(&mut self, addr: u32) -> u16 {
        hword(self.read_byte(addr), self.read_byte(addr + 1))
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        word(self.read_hword(addr), self.read_hword(addr + 2))
    }

    pub fn write_byte(&mut self, addr: u32, _value: u8) {
        match addr {
            _ => (),
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
}
