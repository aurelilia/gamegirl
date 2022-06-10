mod alu;

use crate::{
    gga::GameGirlAdv,
    numutil::{NumExt, U32Ext},
};
use std::fmt::Display;

/// Represents the CPU of the console - an ARM7TDMI.
#[derive(Debug, Clone)]
pub struct CPU {
    pub mode: CPUMode,
    pub regs: Regs,
    pub IE: u16,
    pub IF: u16,
    pub IME: bool,
}

impl CPU {
    fn flag(&self, flag: Flag) -> bool {
        self.regs.cpsr.is_bit(flag as u16)
    }

    fn set_flag(&mut self, flag: Flag, en: bool) {
        self.regs.cpsr = self.regs.cpsr.set_bit(flag as u16, en);
    }

    fn loreg(&mut self, inst: u32, pos: u32) -> u32 {
        self.regs.low[((inst >> pos) & 7) as usize]
    }

    fn loregm(&mut self, inst: u32, pos: u32) -> &mut u32 {
        &mut self.regs.low[((inst >> pos) & 7) as usize]
    }
}

/// CPU state.
#[derive(Debug, Clone)]
pub enum CPUMode {
    Arm,
    Thumb,
}

pub enum Flag {
    Sign = 31,
    Zero = 30,
    Carry = 29,
    Overflow = 28,
}

/// All registers on the ARM7TDMI.
#[derive(Debug, Clone)]
pub struct Regs {
    pub low: [u32; 8],
    pub high: [FiqReg; 5],
    pub sp: ModeReg,
    pub lr: ModeReg,
    pub pc: u32,
    pub cpsr: u32,
    pub spsr: ModeReg,
}

/// A register with values for FIQ and all other modes
#[derive(Debug, Clone, Copy, Default)]
pub struct FiqReg {
    pub reg: u32,
    pub fiq: u32,
}

/// A register with different values for the different CPU modes
#[derive(Debug, Clone, Default)]
pub struct ModeReg {
    pub reg: u32,
    pub fiq: u32,
    pub svc: u32,
    pub abt: u32,
    pub irq: u32,
    pub und: u32,
}

impl GameGirlAdv {
    pub fn execute_inst_thumb(&mut self, inst: u16) {
        let inst = inst as u32;
        match inst & 0xE000 {
            0x0000 => {
                let value = self.cpu.loreg(inst, 3);
                let by = inst.bits(6, 4);
                *self.cpu.loregm(inst, 0) = match inst.bits(11, 2) {
                    0 => self.cpu.lsl(value, by),
                    1 => self.cpu.lsr(value, by),
                    2 => self.cpu.asr(value, by),
                    _ => match inst.bits(9, 2) {
                        0 => {
                            let rn = self.cpu.loreg(inst, 6);
                            self.cpu.add(value, rn, 0)
                        }
                        1 => {
                            let rn = self.cpu.loreg(inst, 6);
                            self.cpu.sub(value, rn, 0)
                        }
                        2 => self.cpu.add(value, inst.bits(6, 2), 0),
                        _ => self.cpu.sub(value, inst.bits(6, 2), 0),
                    },
                }
            }

            0x2000 => {
                let nn = inst.bits(0, 8);
                let rd = self.cpu.loreg(inst, 8);
                *self.cpu.loregm(inst, 8) = match inst.bits(11, 2) {
                    0 => {
                        // MOV
                        self.cpu.set_zn(nn);
                        nn
                    }
                    1 => {
                        // CMP
                        self.cpu.sub(rd, nn, 0);
                        rd
                    }
                    2 => self.cpu.add(rd, nn, 0),
                    _ => self.cpu.sub(rd, nn, 0),
                }
            }

            0x0400 => {
                let rd = self.cpu.loreg(inst, 0);
                let rs = self.cpu.loreg(inst, 3);
                *self.cpu.loregm(inst, 0) = match inst.bits(6, 4) {
                    0x0 => self.cpu.and(rd, rs),
                    0x1 => self.cpu.xor(rd, rs),
                    0x2 => self.cpu.lsl(rd, rs & 0xFF),
                    0x3 => self.cpu.lsr(rd, rs & 0xFF),
                    0x4 => self.cpu.asr(rd, rs & 0xFF),
                    0x5 => self.cpu.add(rd, rs, self.cpu.flag(Flag::Carry) as u32),
                    0x6 => self.cpu.sub(rd, rs, self.cpu.flag(Flag::Carry) as u32),
                    0x7 => self.cpu.ror(rd, rs & 0xFF),
                    0x8 => {
                        // TST
                        self.cpu.and(rd, rs);
                        rd
                    }
                    0x9 => self.cpu.neg(rs),
                    0xA => {
                        // CMP
                        self.cpu.sub(rd, rs, 0);
                        rd
                    }
                    0xB => {
                        // CMN
                        self.cpu.add(rd, rs, 0);
                        rd
                    }
                    0xC => self.cpu.or(rd, rs),
                    0xD => self.cpu.mul(rd, rs),
                    0xE => self.cpu.bit_clear(rd, rs),
                    _ => self.cpu.not(rs),
                }
            }

            _ => Self::log_unknown_opcode(inst),
        }
    }

    fn log_unknown_opcode<T: Display>(code: T) {
        eprintln!("Unknown opcode '{}'", code);
    }
}
