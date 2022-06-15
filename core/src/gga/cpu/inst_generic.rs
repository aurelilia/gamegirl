use crate::gga::{
    cpu::{registers::Flag::*, Cpu},
    Access::NonSeq,
    GameGirlAdv,
};
use std::fmt::UpperHex;

impl GameGirlAdv {
    /// Called by multiple load/store instructions when the Rlist was
    /// empty, which causes R15 to be loaded/stored and Rb to be
    /// incremented/decremented by 0x40.
    pub fn on_empty_rlist(&mut self, rb: u32, str: bool, up: bool) {
        let addr = self.cpu.reg(rb);
        self.cpu.set_reg(rb, Self::mod_with_offs(addr, 0x40, up));

        if str {
            self.write_word(addr, self.cpu.pc + self.cpu.inst_size(), NonSeq);
        } else {
            let val = self.read_word(addr, NonSeq);
            self.cpu.set_pc(val);
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

    pub fn log_unknown_opcode<T: UpperHex>(code: T) {
        eprintln!("Unknown opcode '{:08X}'", code);
    }
}

impl Cpu {
    pub fn eval_condition(&self, cond: u16) -> bool {
        match cond {
            0x0 => self.flag(Zero),                                              // BEQ
            0x1 => !self.flag(Zero),                                             // BNE
            0x2 => self.flag(Carry),                                             // BCS/BHS
            0x3 => !self.flag(Carry),                                            // BCC/BLO
            0x4 => self.flag(Sign),                                              // BMI
            0x5 => !self.flag(Sign),                                             // BPL
            0x6 => self.flag(Overflow),                                          // BVS
            0x7 => !self.flag(Overflow),                                         // BVC
            0x8 => !self.flag(Zero) && self.flag(Carry),                         // BHI
            0x9 => !self.flag(Carry) || self.flag(Zero),                         // BLS
            0xA => self.flag(Zero) == self.flag(Overflow),                       // BGE
            0xB => self.flag(Zero) != self.flag(Overflow),                       // BLT
            0xC => !self.flag(Zero) && (self.flag(Sign) == self.flag(Overflow)), // BGT
            0xD => self.flag(Zero) || (self.flag(Sign) != self.flag(Overflow)),  // BLE
            0xE => true,                                                         // BAL
            _ => false,                                                          // BNV
        }
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
