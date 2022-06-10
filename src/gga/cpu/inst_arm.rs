use crate::gga::cpu::{registers::Flag::*, Cpu};
use crate::gga::GameGirlAdv;
use crate::numutil::{NumExt, U32Ext};
use bitmatch::bitmatch;
use std::fmt::Display;

impl GameGirlAdv {
    #[bitmatch]
    pub fn execute_inst_arm(&mut self, inst: u32) {
        if !self.cpu.eval_condition(inst.bits(28, 4).u16()) {
            return;
        }

        #[bitmatch]
        match inst {
            "101_lnnnnnnnnnnnnnnnnnnnnnnnn" => {
                // TODO sign extend properly i think?
                let nn = n as i32 * 4; // Step 4
                if l == 1 {
                    // BL
                    self.cpu.set_lr(self.cpu.pc + 4);
                } // else: B
                self.cpu.set_pc(self.cpu.pc.wrapping_add_signed(8 + nn));
            }

            "000100101111111111110001_nnnn" => {
                let rn = self.reg(n);
                if rn.is_bit(0) {
                    self.cpu.set_pc(rn - 1);
                    self.cpu.set_flag(Thumb, true);
                } else {
                    self.cpu.set_pc(rn);
                }
            }

            _ => Self::log_unknown_opcode(inst),
        }
    }

    pub fn log_unknown_opcode<T: Display>(code: T) {
        eprintln!("Unknown opcode '{}'", code);
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
}
