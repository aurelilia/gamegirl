use crate::gga::cpu::{registers::Flag::*, Cpu};
use crate::gga::GameGirlAdv;
use crate::numutil::{NumExt, U32Ext};
use bitmatch::bitmatch;
use std::fmt::UpperHex;

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

            "1111_????????????????????????" => {
                // TODO: what the fuck is a SWI
            }

            "00010_0001111dddd000000000000" => self.cpu.set_reg(d, self.cpu.cpsr),
            "00010_1001111dddd000000000000" => self.cpu.set_reg(d, self.cpu.spsr()),

            "00010_d10fsxc111100000000mmmm" => {
                // TODO: MSR
            }
            "00110_d10fsxc1111mmmmnnnnnnnn" => {
                // TODO: MSR
            }

            "000_000lcddddnnnnssss1001mmmm" => {
                // MUL/MLA
                let cpsr = self.cpu.cpsr;
                let mut res = self.cpu.mul(self.cpu.reg(m), self.cpu.reg(s));
                if l == 1 {
                    // MLA
                    res += self.cpu.reg(n);
                }

                self.cpu.set_reg(d, res);
                if c == 0 {
                    // Restore CPSR if we weren't supposed to set flags
                    self.cpu.cpsr = cpsr;
                }
            }

            "000_oooocnnnnddddaaaaattrmmmm" => {
                // ALU with register
                let rm = self.cpu.reg(m);
                let second_op = if r + a == 0 {
                    // Special case: no shift
                    rm
                } else {
                    let shift_amount = if r == 0 {
                        // Shift by imm
                        a
                    } else {
                        // Shift by reg
                        self.cpu.reg(a >> 1)
                    };
                    match t {
                        0 => self.cpu.lsl(rm, shift_amount),
                        1 => self.cpu.lsr(rm, shift_amount),
                        2 => self.cpu.asr(rm, shift_amount),
                        _ => self.cpu.ror(rm, shift_amount),
                    }
                };
                self.exec_alu(o, self.cpu.reg(n), second_op, d, c == 1);
            }

            "001_oooocnnnnddddssssnnnnnnnn" => {
                // ALU with immediate
                let second_op = self.cpu.ror(n, s << 1);
                self.exec_alu(o, self.cpu.reg(n), second_op, d, c == 1);
            }

            _ => Self::log_unknown_opcode(inst),
        }
    }

    fn exec_alu(&mut self, op: u32, a: u32, b: u32, dest: u32, flags: bool) {
        let cpsr = self.cpu.cpsr;
        let d = self.cpu.reg(dest);

        let value = match op {
            0x0 => self.cpu.and(a, b),
            0x1 => self.cpu.xor(a, b),
            0x2 => self.cpu.sub(a, b, 0),
            0x3 => self.cpu.sub(b, a, 0),
            0x4 => self.cpu.add(a, b, 0),
            0x5 => self.cpu.add(a, b, self.cpu.flag(Carry) as u32),
            0x6 => self.cpu.sub(a, b, self.cpu.flag(Carry) as u32),
            0x7 => self.cpu.sub(b, a, self.cpu.flag(Carry) as u32),
            0x8 => {
                // TST
                self.cpu.and(a, b);
                d
            }
            0x9 => {
                // TEQ
                self.cpu.xor(a, b);
                d
            }
            0xA => {
                // CMP
                self.cpu.sub(a, b, 0);
                d
            }
            0xB => {
                // CMN
                self.cpu.add(a, b, 0);
                d
            }
            0xC => self.cpu.or(a, b),
            0xD => b, // MOV
            0xE => self.cpu.bit_clear(a, b),
            _ => self.cpu.not(b),
        };

        self.cpu.set_reg(dest, value);
        if !flags {
            // Restore CPSR if we weren't supposed to set flags
            self.cpu.cpsr = cpsr;
        }
    }

    #[bitmatch]
    pub fn get_mnemonic_arm(inst: u32) -> String {
        let co = Cpu::condition_mnemonic(inst.bits(28, 4).u16());
        #[bitmatch]
        match inst {
            "101_0nnnnnnnnnnnnnnnnnnnnnnnn" => format!("b{co} +{}", (n << 2) + 8),
            "101_1nnnnnnnnnnnnnnnnnnnnnnnn" => format!("bl{co} +{}", (n << 2) + 8),
            "000100101111111111110001_nnnn" => format!("bx{co} +{}", (n << 2) + 8),
            "1111_nnnnnnnnnnnnnnnnnnnnnnnn" => format!("swi{co} {n}"),

            "00010_0001111dddd000000000000" => format!("mrs{co} r{d}, cpsr"),
            "00010_1001111dddd000000000000" => format!("mrs{co} r{d}, spsr"),
            "00010_d10fsxc111100000000mmmm" => format!("msr{co} reg (todo)"),
            "00110_d10fsxc1111mmmmnnnnnnnn" => format!("msr{co} imm (todo)"),

            "000_0000cdddd????ssss1001mmmm" => format!("mul{co} r{d}, r{m}, r{s}, ({c})"),
            "000_0001cddddnnnnssss1001mmmm" => format!("mul{co} r{d}, r{m}, r{s}, r{n} ({c})"),
            "000_oooocnnnnddddaaaaattrmmmm" => {
                let ty = match t {
                    0 => "lsl",
                    1 => "lsr",
                    2 => "asr",
                    _ => "ror",
                };
                let shift = match (r, t, a) {
                    (0, 0, 0) => "".to_string(),
                    (0, _, _) => format!("({ty} #{a})"),
                    _ => format!("({ty} r{})", a >> 1),
                };
                let op = Self::alu_mnemonic(o);
                format!("{op}{c} r{d}, r{n}, r{m} {shift} ({c})")
            }
            "001_oooocnnnnddddssssnnnnnnnn" => {
                let op = Self::alu_mnemonic(o);
                format!("{op}{co} r{d}, r{n}, (#{n} ROR {s}) ({c})")
            }

            _ => format!("{:08X}??", inst),
        }
    }

    fn alu_mnemonic(opt: u32) -> &'static str {
        match opt {
            0x0 => "and",
            0x1 => "eor",
            0x2 => "sub",
            0x3 => "rsb",
            0x4 => "add",
            0x5 => "adc",
            0x6 => "sbc",
            0x7 => "rsc",
            0x8 => "tst",
            0x9 => "teq",
            0xA => "cmp",
            0xB => "cmn",
            0xC => "orr",
            0xD => "mul",
            0xE => "bic",
            _ => "mvn",
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
