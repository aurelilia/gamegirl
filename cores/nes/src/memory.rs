// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::{hword, NumExt};

use crate::{cartridge::Cartridge, cpu::Reg::*, Nes};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    iram: [u8; 0x800],
    ppu_regs: [u8; 0x8],
    other_regs: [u8; 0x15],
}

impl Nes {
    pub fn read_imm(&mut self) -> u8 {
        let value = self.read(self.cpu.pc);
        self.cpu.pc += 1;
        value
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        self.advance_clock(1);
        self.get(addr)
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        self.advance_clock(1);
        self.set(addr, value);
    }

    pub fn push(&mut self, value: u8) {
        let stack = self.cpu.get(S);
        self.write(hword(0x01, stack), value);
        self.cpu.set(S, stack.wrapping_sub(1));
        self.advance_clock(1);
    }

    pub fn pop(&mut self) -> u8 {
        self.advance_clock(2);
        let stack = self.cpu.get(S).wrapping_add(1);
        self.cpu.set(S, stack);
        self.read(hword(0x01, stack))
    }

    pub fn get(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.mem.iram[addr.us() & 0x7FF],
            0x2000..=0x3FFF => self.mem.ppu_regs[addr.us() & 0x8],
            0x4000..=0x4015 => self.mem.other_regs[addr.us() - 0x4000],
            0x4016 => self.joypad.read() | 0x40,
            0x4020..=0xFFFF => Cartridge::read(self, addr),
            _ => 0xFF,
        }
    }

    pub fn set(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.mem.iram[addr.us() & 0x7FF] = value,
            0x2000..=0x3FFF => self.mem.ppu_regs[addr.us() & 0x8] = value,
            0x4000..=0x4015 => self.mem.other_regs[addr.us() - 0x4000] = value,
            0x4016 => self.joypad.write(value),
            0x4020..=0xFFFF => Cartridge::write(self, addr, value),
            _ => (),
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            iram: [0; 0x800],
            ppu_regs: [0; 0x8],
            other_regs: [0; 0x15],
        }
    }
}
