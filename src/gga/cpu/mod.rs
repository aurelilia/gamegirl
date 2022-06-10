mod alu;

use crate::{
    gga::GameGirlAdv,
    numutil::{NumExt, U32Ext},
};
use std::fmt::Display;

/// Macro for creating accessors for mode-dependent registers.
macro_rules! mode_reg {
    ($reg:ident, $m:ident) => {
        fn $reg(&self) -> u32 {
            // TODO mode
            self.regs.$reg.reg
        }

        fn $m(&mut self) -> &mut u32 {
            // TODO mode
            &mut self.regs.$reg.reg
        }
    };
}

/// Represents the CPU of the console - an ARM7TDMI.
pub struct Cpu {
    pub mode: CpuMode,
    pub regs: Regs,
    pub ie: u16,
    pub if_: u16,
    pub ime: bool,
}

impl Cpu {
    fn flag(&self, flag: Flag) -> bool {
        self.regs.cpsr.is_bit(flag as u16)
    }

    fn set_flag(&mut self, flag: Flag, en: bool) {
        self.regs.cpsr = self.regs.cpsr.set_bit(flag as u16, en);
    }

    fn adj_pc(&self) -> u32 {
        (self.regs.pc + 4) & !2
    }

    mode_reg!(sp, spm);
    mode_reg!(lr, lrm);
    mode_reg!(spsr, spsrm);

    fn loreg(&mut self, inst: u32, pos: u32) -> u32 {
        self.regs.low[((inst >> pos) & 7) as usize]
    }

    fn loregm(&mut self, inst: u32, pos: u32) -> &mut u32 {
        &mut self.regs.low[((inst >> pos) & 7) as usize]
    }

    fn hireg(&mut self, inst: u32, pos: u32, bit: u16) -> u32 {
        if inst.is_bit(bit) {
            todo!()
        } else {
            self.loreg(inst, pos)
        }
    }

    fn hiregm(&mut self, inst: u32, pos: u32, bit: u16) -> &mut u32 {
        if inst.is_bit(bit) {
            todo!()
        } else {
            self.loregm(inst, pos)
        }
    }
}

/// CPU state.
pub enum CpuMode {
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
#[derive(Clone, Copy, Default)]
pub struct FiqReg {
    pub reg: u32,
    pub fiq: u32,
}

/// A register with different values for the different CPU modes
#[derive(Clone, Default)]
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
            // THUMB.1/2
            _ if inst.bits(13, 3) == 0b000 => {
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

            // THUMB.3
            _ if inst.bits(13, 3) == 0b001 => {
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

            // THUMB.4
            _ if inst.bits(10, 6) == 0b010000 => {
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

            // THUMB.5
            _ if inst.bits(10, 6) == 0b010001 => {
                let rd = self.cpu.hireg(inst, 0, 7);
                let rs = self.cpu.hireg(inst, 3, 6);
                match inst.bits(8, 2) {
                    0 => *self.cpu.hiregm(inst, 0, 7) = self.cpu.add(rd, rs, 0),
                    1 => {
                        // CMP
                        self.cpu.sub(rd, rs, 0);
                    }
                    2 => *self.cpu.hiregm(inst, 0, 7) = rs, // MOV

                    // TODO: Switch to ARM mode
                    _ if !inst.is_bit(7) => self.cpu.regs.pc = rs, // BX
                    _ => self.cpu.regs.pc = rs,                    // BLX
                }
            }

            // THUMB.6
            _ if inst.bits(11, 5) == 0b01001 => {
                // LDR
                let addr = self.cpu.adj_pc();
                let addr = addr + inst.bits(0, 8); // NN
                *self.cpu.loregm(inst, 8) = self.read_word(addr);
            }

            // THUMB.7/8
            _ if inst.bits(12, 4) == 0b0101 => {
                let rb = self.cpu.loreg(inst, 3);
                let ro = self.cpu.loreg(inst, 6);
                let rd = self.cpu.loreg(inst, 0);
                let addr = rb + ro;

                match inst.bits(9, 3) {
                    0 => self.write_word(addr, rd),        // STR
                    1 => self.write_hword(addr, rd.u16()), // STRH
                    2 => self.write_byte(addr, rd.u8()),   // STRB
                    3 => *self.cpu.loregm(inst, 0) = self.read_byte(addr) as i8 as i32 as u32, // LDSB
                    4 => *self.cpu.loregm(inst, 0) = self.read_word(addr), // LDR
                    5 => *self.cpu.loregm(inst, 0) = self.read_hword(addr).u32(), // LDRH
                    6 => *self.cpu.loregm(inst, 0) = self.read_byte(addr).u32(), // LDRB
                    _ => *self.cpu.loregm(inst, 0) = self.read_hword(addr) as i16 as i32 as u32, // LDSH
                }
            }

            // THUMB.9
            _ if inst.bits(13, 3) == 0b011 => {
                let rb = self.cpu.loreg(inst, 3);
                let ro = inst.bits(6, 5);
                let rd = self.cpu.loreg(inst, 0);

                match inst.bits(11, 2) {
                    0 => self.write_word(rb + (ro << 2), rd), // STR
                    1 => *self.cpu.loregm(inst, 0) = self.read_word(rb + (ro << 2)), // LDR
                    2 => self.write_byte(rb + ro, rd.u8()),   // STRB
                    _ => *self.cpu.loregm(inst, 0) = self.read_byte(rb + ro).u32(), // LDRB
                }
            }

            // THUMB.10
            _ if inst.bits(12, 4) == 0b1000 => {
                let rb = self.cpu.loreg(inst, 3);
                let ro = inst.bits(6, 5) << 1; // Step 2
                let rd = self.cpu.loreg(inst, 0);
                let addr = rb + ro;

                if !inst.is_bit(11) {
                    self.write_hword(addr, rd.u16());
                } else {
                    *self.cpu.loregm(inst, 0) = self.read_hword(addr).u32();
                }
            }

            // THUMB.11
            _ if inst.bits(12, 4) == 0b1001 => {
                let nn = inst.bits(0, 8);
                let rd = self.cpu.loreg(inst, 8);
                let addr = self.cpu.sp() + nn;

                if !inst.is_bit(11) {
                    self.write_word(addr, rd);
                } else {
                    *self.cpu.loregm(inst, 8) = self.read_word(addr);
                }
            }

            // THUMB.12
            _ if inst.bits(12, 4) == 0b1010 => {
                let nn = inst.bits(0, 8) << 2; // Step 4
                *self.cpu.loregm(inst, 8) = if !inst.is_bit(11) {
                    self.cpu.adj_pc() + nn
                } else {
                    self.cpu.sp() + nn
                };
            }

            // THUMB.13
            _ if inst.bits(8, 8) == 0b10110000 => {
                let nn = inst.bits(0, 6) << 2; // Step 4
                *self.cpu.spm() = if !inst.is_bit(7) {
                    self.cpu.sp() + nn
                } else {
                    self.cpu.sp() - nn
                };
            }

            // THUMB.14
            _ if inst.bits(12, 4) == 0b1011 => {
                let pclr_bit = inst.is_bit(8);
                let mut sp = self.cpu.sp();
                if !inst.is_bit(11) {
                    // PUSH
                    for reg in 0..8 {
                        if inst.is_bit(reg) {
                            self.write_word(sp, self.cpu.regs.low[reg.us()]);
                            sp -= 4;
                        }
                    }
                    if pclr_bit {
                        self.write_word(sp, self.cpu.lr());
                        sp -= 4;
                    }
                } else {
                    // POP
                    for reg in 0..8 {
                        if inst.is_bit(reg) {
                            self.cpu.regs.low[reg.us()] = self.read_word(sp);
                            sp -= 4;
                        }
                    }
                    if pclr_bit {
                        self.cpu.regs.pc = self.read_word(sp);
                        sp -= 4;
                    }
                }
                *self.cpu.spm() = sp;
            }

            // THUMB.15
            _ if inst.bits(12, 4) == 0b1100 => {
                let mut rb = self.cpu.loreg(inst, 8);
                if !inst.is_bit(11) {
                    // STMIA
                    for reg in 0..8 {
                        if inst.is_bit(reg) {
                            self.write_word(rb, self.cpu.regs.low[reg.us()]);
                            rb += 4;
                        }
                    }
                } else {
                    // LDMIA
                    for reg in 0..8 {
                        if inst.is_bit(reg) {
                            self.cpu.regs.low[reg.us()] = self.read_word(rb);
                            rb += 4;
                        }
                    }
                }
                *self.cpu.loregm(inst, 8) = rb;
            }

            // THUMB.16
            _ if inst.bits(12, 4) == 0b1101 => {
                let nn = (inst.bits(0, 8).u8() as i8 as i32) * 2; // Step 2
                let condition = match inst.bits(8, 4) {
                    0x0 => self.cpu.flag(Flag::Zero),      // BEQ
                    0x1 => !self.cpu.flag(Flag::Zero),     // BNE
                    0x2 => self.cpu.flag(Flag::Carry),     // BCS/BHS
                    0x3 => !self.cpu.flag(Flag::Carry),    // BCC/BLO
                    0x4 => self.cpu.flag(Flag::Sign),      // BMI
                    0x5 => !self.cpu.flag(Flag::Sign),     // BPL
                    0x6 => self.cpu.flag(Flag::Overflow),  // BVS
                    0x7 => !self.cpu.flag(Flag::Overflow), // BVC
                    0x8 => !self.cpu.flag(Flag::Zero) && self.cpu.flag(Flag::Carry), // BHI
                    0x9 => !self.cpu.flag(Flag::Carry) || self.cpu.flag(Flag::Zero), // BLS
                    0xA => self.cpu.flag(Flag::Zero) == self.cpu.flag(Flag::Overflow), // BGE
                    0xB => self.cpu.flag(Flag::Zero) != self.cpu.flag(Flag::Overflow), // BLT
                    0xC => {
                        // BGT
                        !self.cpu.flag(Flag::Zero)
                            && (self.cpu.flag(Flag::Sign) == self.cpu.flag(Flag::Overflow))
                    }
                    0xD => {
                        // BLE
                        self.cpu.flag(Flag::Zero)
                            || (self.cpu.flag(Flag::Sign) != self.cpu.flag(Flag::Overflow))
                    }
                    _ => false,
                };
                if condition {
                    self.cpu.regs.pc = self.cpu.adj_pc().wrapping_add_signed(nn);
                }
            }

            // THUMB.18
            _ if inst.bits(11, 5) == 0b11100 => {
                let nn = (inst.bits(0, 10).u16() as i16 as i32) * 2; // Step 2
                self.cpu.regs.pc = self.cpu.adj_pc().wrapping_add_signed(nn);
            }

            // THUMB.19
            _ if inst.bits(11, 5) == 0b11110 => {
                let nn = inst.bits(0, 11);
                *self.cpu.lrm() = self.cpu.regs.pc + 4 + (nn << 12);
            }
            _ if inst.bits(11, 3) == 0b111 => {
                let nn = inst.bits(0, 11);
                *self.cpu.lrm() = (self.cpu.regs.pc + 2) | 1;
                self.cpu.regs.pc = self.cpu.lr() + (nn << 1);

                if !inst.is_bit(12) {
                    // TODO: switch to ARM
                }
            }

            _ => Self::log_unknown_opcode(inst),
        }
    }

    fn log_unknown_opcode<T: Display>(code: T) {
        eprintln!("Unknown opcode '{}'", code);
    }
}
