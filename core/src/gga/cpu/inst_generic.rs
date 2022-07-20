// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt::UpperHex;

use crate::{
    gga::{
        cpu::{Cpu, Exception},
        Access,
        Access::NonSeq,
        GameGirlAdv,
    },
    numutil::NumExt,
};

impl GameGirlAdv {
    pub fn swi(&mut self) {
        Cpu::exception_occurred(self, Exception::Swi);
        self.memory.bios_value = 0xE3A02004;
    }

    /// Called by multiple load/store instructions when the Rlist was
    /// empty, which causes R15 to be loaded/stored and Rb to be
    /// incremented/decremented by 0x40.
    pub fn on_empty_rlist(&mut self, rb: u32, str: bool, up: bool, before: bool) {
        let addr = self.cpu.reg(rb);
        self.set_reg(rb, Self::mod_with_offs(addr, 0x40, up));

        if str {
            let addr = match (up, before) {
                (true, true) => addr + 4,
                (true, false) => addr,
                (false, true) => addr - 0x40,
                (false, false) => addr - 0x3C,
            };
            self.write_word(addr, self.cpu.pc() + self.cpu.inst_size(), NonSeq);
        } else {
            let val = self.read_word(addr, NonSeq);
            self.set_pc(val);
        }
    }

    /// Modify a value with an offset, either adding or subtracting.
    pub fn mod_with_offs(value: u32, offs: u32, up: bool) -> u32 {
        if up {
            value.wrapping_add(offs)
        } else {
            value.wrapping_sub(offs)
        }
    }

    pub fn idle_nonseq(&mut self) {
        self.add_i_cycles(1);
        self.cpu.access_type = Access::NonSeq;
    }

    pub fn mul_wait_cycles(&mut self, mut value: u32, signed: bool) {
        self.idle_nonseq();
        let mut mask = 0xFFFF_FF00;
        loop {
            value &= mask;
            if value == 0 || (signed && value == mask) {
                break;
            }
            self.add_i_cycles(1);
            mask <<= 8;
        }
    }

    pub const fn lut_span<T: Copy>(lut: &mut [T], idx: usize, size: usize, handler: T) {
        let inst = 8 - size;
        let start = idx << inst;

        let until = 1 << inst;
        let mut idx = 0;
        while idx < until {
            lut[start | idx] = handler;
            idx += 1;
        }
    }

    pub fn log_unknown_opcode<T: UpperHex>(code: T) {
        eprintln!("Unknown opcode '{:08X}'", code);
    }
}

impl Cpu {
    pub fn eval_condition(&self, cond: u16) -> bool {
        // This condition table is taken from mGBA sources, which are licensed under
        // MPL2 at https://github.com/mgba-emu/mgba
        // Thank you to endrift and other mGBA contributors!
        const COND_MASKS: [u16; 16] = [
            0xF0F0, // EQ [-Z--]
            0x0F0F, // NE [-z--]
            0xCCCC, // CS [--C-]
            0x3333, // CC [--c-]
            0xFF00, // MI [N---]
            0x00FF, // PL [n---]
            0xAAAA, // VS [---V]
            0x5555, // VC [---v]
            0x0C0C, // HI [-zC-]
            0xF3F3, // LS [-Z--] || [--c-]
            0xAA55, // GE [N--V] || [n--v]
            0x55AA, // LT [N--v] || [n--V]
            0x0A05, // GT [Nz-V] || [nz-v]
            0xF5FA, // LE [-Z--] || [Nz-v] || [nz-V]
            0xFFFF, // AL [----]
            0x0000, // NV
        ];

        let flags = self.cpsr >> 28;
        (COND_MASKS[cond.us()] & (1 << flags)) != 0
    }

    pub fn condition_mnemonic(cond: u16) -> &'static str {
        match cond {
            0x0 => "eq",
            0x1 => "ne",
            0x2 => "cs",
            0x3 => "cc",
            0x4 => "mi",
            0x5 => "pl",
            0x6 => "vs",
            0x7 => "vc",
            0x8 => "hi",
            0x9 => "ls",
            0xA => "ge",
            0xB => "lt",
            0xC => "gt",
            0xD => "le",
            0xE => "",
            _ => "nv",
        }
    }
}
