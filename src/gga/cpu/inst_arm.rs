use crate::gga::cpu::registers::Context;
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

            "00010_d10f??c111100000000mmmm" => self.msr(self.cpu.reg(m), f == 1, c == 1, d == 1),
            "00110_d10f??c1111mmmmnnnnnnnn" => {
                let imm = self.cpu.ror_s0(n, m << 1);
                self.msr(imm, f == 1, c == 1, d == 1);
            }

            "00010_b00nnnndddd00001001mmmm" => {
                let addr = self.reg(n);
                let mem_value = if b == 1 {
                    self.read_byte(addr).u32()
                } else {
                    self.read_word(addr)
                };
                let reg = self.reg(m);
                if b == 1 {
                    self.write_byte(addr, reg.u8())
                } else {
                    self.write_word(addr, reg)
                }
                self.cpu.set_reg(d, mem_value);
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
                let shift_amount = if r == 0 {
                    // Shift by imm
                    a
                } else {
                    // Shift by reg
                    self.cpu.reg(a >> 1)
                };
                let second_op = self.shifted_op(rm, t, shift_amount);
                self.alu(o, self.cpu.reg(n), second_op, d, c == 1);
            }

            "001_oooocnnnnddddssssmmmmmmmm" => {
                // ALU with immediate
                let second_op = self.cpu.ror(m, s << 1);
                self.alu(o, self.cpu.reg(n), second_op, d, c == 1);
            }

            "01_0pubwlnnnnddddmmmmmmmmmmmm" => {
                // LDR/STR with imm
                let width = if b == 1 { 1 } else { 4 };
                self.ldrstr(p == 1, u == 1, width, (p == 1) || (w == 1), l == 0, n, d, m);
            }
            "01_1pubwlnnnnddddssssstt0mmmm" => {
                // LDR/STR with reg
                let offs = self.shifted_op(self.cpu.reg(m), t, s);
                let width = if b == 1 { 1 } else { 4 };
                self.ldrstr(
                    p == 1,
                    u == 1,
                    width,
                    (p == 1) || (w == 1),
                    l == 0,
                    n,
                    d,
                    offs,
                );
            }

            "000_pu1wlnnnnddddiiii1011iiii" => {
                // LDRH/STRH with imm
                self.ldrstr(p == 1, u == 1, 2, (p == 1) || (w == 1), l == 0, n, d, i);
            }
            "000_pu0wlnnnndddd00001011mmmm" => {
                // LDRH/STRH with reg
                self.ldrstr(
                    p == 1,
                    u == 1,
                    2,
                    (p == 1) || (w == 1),
                    l == 0,
                    n,
                    d,
                    self.cpu.reg(m),
                );
            }

            "000_pu1w1nnnnddddiiii1101iiii" => {
                // LDRSB with imm
                self.ldrstr(p == 1, u == 1, 1, (p == 1) || (w == 1), false, n, d, i);
                self.cpu.set_reg(d, self.reg(d).u8() as i8 as i32 as u32);
            }
            "000_pu0w1nnnndddd00001101mmmm" => {
                // LDRSB with reg
                self.ldrstr(
                    p == 1,
                    u == 1,
                    1,
                    (p == 1) || (w == 1),
                    false,
                    n,
                    d,
                    self.cpu.reg(m),
                );
                self.cpu.set_reg(d, self.reg(d).u8() as i8 as i32 as u32);
            }
            "000_pu1w1nnnnddddiiii1111iiii" => {
                // LDRSH with imm
                self.ldrstr(p == 1, u == 1, 1, (p == 1) || (w == 1), false, n, d, i);
                self.cpu.set_reg(d, self.reg(d).u16() as i16 as i32 as u32);
            }
            "000_pu0w1nnnndddd00001111mmmm" => {
                // LDRSH with reg
                self.ldrstr(
                    p == 1,
                    u == 1,
                    1,
                    (p == 1) || (w == 1),
                    false,
                    n,
                    d,
                    self.cpu.reg(m),
                );
                self.cpu.set_reg(d, self.reg(d).u16() as i16 as i32 as u32);
            }

            _ => Self::log_unknown_opcode(inst),
        }
    }

    fn alu(&mut self, op: u32, a: u32, b: u32, dest: u32, flags: bool) {
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

    fn msr(&mut self, src: u32, flags: bool, ctrl: bool, spsr: bool) {
        let mut dest = if spsr { self.cpu.spsr() } else { self.cpu.cpsr };

        if flags {
            dest = (dest & 0x00FFFFFF) | (src & 0x00FFFFFF)
        };
        if ctrl && self.cpu.context() != Context::User {
            dest = (dest & 0xFFFFFF00) | (src & 0xFF)
        };
        // Thumb flag may not be changed
        dest = dest.set_bit(5, false);

        if spsr {
            self.cpu.set_spsr(dest);
        } else {
            self.cpu.cpsr = dest;
        }
    }

    fn ldrstr(
        &mut self,
        post: bool,
        up: bool,
        width: u32,
        writeback: bool,
        str: bool,
        n: u32,
        d: u32,
        offs: u32,
    ) {
        let mut addr = self.cpu.reg(n);
        if !post {
            addr = Self::mod_with_offs(addr, offs, up);
        }

        let mut value = match (str, width) {
            (true, 4) => self.reg(d),
            (true, 2) => self.reg(d) & 0xFFFF,
            (true, _) => self.reg(d) & 0xFF,
            (false, 4) => self.read_word(addr),
            (false, 2) => self.read_hword(addr).u32(),
            (false, _) => self.read_byte(addr).u32(),
        };
        if post {
            value = Self::mod_with_offs(value, offs, up);
        }
        if writeback {
            self.cpu.set_reg(n, value);
        }

        match (str, width) {
            (true, 4) => self.write_word(addr, value),
            (true, 2) => self.write_hword(addr, value.u16()),
            (true, _) => self.write_byte(addr, value.u8()),
            (false, _) => self.cpu.set_reg(d, value),
        };
    }

    fn mod_with_offs(value: u32, offs: u32, up: bool) -> u32 {
        if up {
            value + offs
        } else {
            value - offs
        }
    }

    fn shifted_op(&mut self, nn: u32, op: u32, shift_amount: u32) -> u32 {
        if op + shift_amount == 0 {
            // Special case: no shift
            nn
        } else {
            match op {
                0 => self.cpu.lsl(nn, shift_amount),
                1 => self.cpu.lsr(nn, shift_amount),
                2 => self.cpu.asr(nn, shift_amount),
                _ => self.cpu.ror(nn, shift_amount),
            }
        }
    }

    #[bitmatch]
    #[allow(unused_variables)]
    pub fn get_mnemonic_arm(inst: u32) -> String {
        let co = Cpu::condition_mnemonic(inst.bits(28, 4).u16());
        #[bitmatch]
        match inst {
            "101_0nnnnnnnnnnnnnnnnnnnnnnnn" => format!("b{co} +{}", (n << 2) + 8),
            "101_1nnnnnnnnnnnnnnnnnnnnnnnn" => format!("bl{co} +{}", (n << 2) + 8),
            "000100101111111111110001_nnnn" => format!("bx{co} +{}", (n << 2) + 8),
            "1111_nnnnnnnnnnnnnnnnnnnnnnnn" => format!("swi{co} {n}"),

            "00010_000nnnndddd00001001mmmm" => format!("swp{co} r{d}, r{m}, [r{n}]"),
            "00010_100nnnndddd00001001mmmm" => format!("swpb{co} r{d}, r{m}, [r{n}]"),

            "00010_0001111dddd000000000000" => format!("mrs{co} r{d}, cpsr"),
            "00010_1001111dddd000000000000" => format!("mrs{co} r{d}, spsr"),
            "00010_d10fsxc111100000000mmmm" => format!("msr{co} reg (todo)"),
            "00110_d10fsxc1111mmmmnnnnnnnn" => format!("msr{co} imm (todo)"),

            "000_0000cdddd????ssss1001mmmm" => format!("mul{co} r{d}, r{m}, r{s}, ({c})"),
            "000_0001cddddnnnnssss1001mmmm" => format!("mul{co} r{d}, r{m}, r{s}, r{n} ({c})"),
            "000_oooocnnnnddddaaaaattrmmmm" => {
                let shift = Self::shift_mnemonic(r, t, a);
                let op = Self::alu_mnemonic(o);
                match o {
                    0x8..=0xB => format!("{op}{c} r{n}, r{m} {shift} ({c})"),
                    0xD | 0xF => format!("{op}{c} r{d}, r{m} {shift} ({c})"),
                    _ => format!("{op}{c} r{d}, r{n}, r{m} {shift} ({c})"),
                }
            }
            "001_oooocnnnnddddssssmmmmmmmm" => {
                let op = Self::alu_mnemonic(o);
                match (o, s) {
                    (0x8..=0xB, 0) => format!("{op}{co} r{d}, r{n}, #{m} ({c})"),
                    (0x8..=0xB, _) => format!("{op}{co} r{d}, r{n}, (#{m} ROR {s}) ({c})"),
                    (0xD | 0xF, 0) => format!("{op}{co} r{d}, #{m} ({c})"),
                    (0xD | 0xF, _) => format!("{op}{co} r{d}, (#{m} ROR {s}) ({c})"),
                    (_, 0) => format!("{op}{co} r{d}, r{n}, #{m} ({c})"),
                    _ => format!("{op}{co} r{d}, r{n}, (#{m} ROR {s}) ({c})"),
                }
            }

            "01_0000?0nnnnddddmmmmmmmmmmmm" => format!("str{co} r{d}, [r{n} -{m}]"),
            "01_0100?0nnnnddddmmmmmmmmmmmm" => format!("str{co} r{d}, [r{n}], -{m}"),
            "01_0010?0nnnnddddmmmmmmmmmmmm" => format!("str{co} r{d}, [r{n} +{m}]"),
            "01_0110?0nnnnddddmmmmmmmmmmmm" => format!("str{co} r{d}, [r{n}], +{m}"),
            "01_0001?0nnnnddddmmmmmmmmmmmm" => format!("strb{co} r{d}, [r{n} -{m}]"),
            "01_0101?0nnnnddddmmmmmmmmmmmm" => format!("strb{co} r{d}, [r{n}], -{m}"),
            "01_0011?0nnnnddddmmmmmmmmmmmm" => format!("strb{co} r{d}, [r{n} +{m}]"),
            "01_0111?0nnnnddddmmmmmmmmmmmm" => format!("strb{co} r{d}, [r{n}], +{m}"),
            "01_0000?1nnnnddddmmmmmmmmmmmm" => format!("ldr{co} r{d}, [r{n} -{m}]"),
            "01_0100?1nnnnddddmmmmmmmmmmmm" => format!("ldr{co} r{d}, [r{n}], -{m}"),
            "01_0010?1nnnnddddmmmmmmmmmmmm" => format!("ldr{co} r{d}, [r{n} +{m}]"),
            "01_0110?1nnnnddddmmmmmmmmmmmm" => format!("ldr{co} r{d}, [r{n}], +{m}"),
            "01_0001?1nnnnddddmmmmmmmmmmmm" => format!("ldrb{co} r{d}, [r{n} -{m}]"),
            "01_0101?1nnnnddddmmmmmmmmmmmm" => format!("ldrb{co} r{d}, [r{n}], -{m}"),
            "01_0011?1nnnnddddmmmmmmmmmmmm" => format!("ldrb{co} r{d}, [r{n} +{m}]"),
            "01_0111?1nnnnddddmmmmmmmmmmmm" => format!("ldrb{co} r{d}, [r{n}], +{m}"),
            "01_1pubwlnnnnddddssssstt0mmmm" => {
                let shift = Self::shift_type_mnemonic(t);
                let u = if u == 1 { "+" } else { "-" };
                let b = if b == 1 { "b" } else { "" };
                let op = if l == 1 { "ldr" } else { "str" };
                if p == 1 {
                    format!("{op}{b}{co} r{d}, [r{n} {u}(r{m} {shift} {s})]")
                } else {
                    format!("{op}{b}{co} r{d}, [r{n}], {u}(r{m} {shift} {s})")
                }
            }

            "000_001?0nnnnddddiiii1011iiii" => format!("strh{co} r{d}, [r{n} -{i}]"),
            "000_101?0nnnnddddiiii1011iiii" => format!("strh{co} r{d}, [r{n}], -{i}"),
            "000_011?0nnnnddddiiii1011iiii" => format!("strh{co} r{d}, [r{n} +{i}]"),
            "000_111?0nnnnddddiiii1011iiii" => format!("strh{co} r{d}, [r{n}], +{i}"),
            "000_001?1nnnnddddiiii1011iiii" => format!("ldrh{co} r{d}, [r{n} -{i}]"),
            "000_101?1nnnnddddiiii1011iiii" => format!("ldrh{co} r{d}, [r{n}], -{i}"),
            "000_011?1nnnnddddiiii1011iiii" => format!("ldrh{co} r{d}, [r{n} +{n}]"),
            "000_111?1nnnnddddiiii1011iiii" => format!("ldrh{co} r{d}, [r{n}], +{i}"),
            "000_001?1nnnnddddiiii1101iiii" => format!("ldrsb{co} r{d}, [r{n} -{i}]"),
            "000_101?1nnnnddddiiii1101iiii" => format!("ldrsb{co} r{d}, [r{n}], -{i}"),
            "000_011?1nnnnddddiiii1101iiii" => format!("ldrsb{co} r{d}, [r{n} +{n}]"),
            "000_111?1nnnnddddiiii1101iiii" => format!("ldrsb{co} r{d}, [r{n}], +{i}"),
            "000_001?1nnnnddddiiii1111iiii" => format!("ldrsh{co} r{d}, [r{n} -{i}]"),
            "000_101?1nnnnddddiiii1111iiii" => format!("ldrsh{co} r{d}, [r{n}], -{i}"),
            "000_011?1nnnnddddiiii1111iiii" => format!("ldrsh{co} r{d}, [r{n} +{n}]"),
            "000_111?1nnnnddddiiii1111iiii" => format!("ldrsh{co} r{d}, [r{n}], +{i}"),
            "000_pu0wlnnnndddd00001oo1mmmm" => {
                let u = if u == 1 { "+" } else { "-" };
                let op = if l == 1 {
                    match o {
                        1 => "ldrh",
                        2 => "ldrsb",
                        3 => "ldrsh",
                        _ => "?",
                    }
                } else {
                    "strh"
                };
                if p == 1 {
                    format!("{op}{co} r{d}, [r{n} {u}r{m}]")
                } else {
                    format!("{op}{co} r{d}, [r{n}], {u}r{m}")
                }
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
            0xD => "mov",
            0xE => "bic",
            _ => "mvn",
        }
    }

    fn shift_mnemonic(r: u32, t: u32, a: u32) -> String {
        let ty = Self::shift_type_mnemonic(t);
        match (r, t, a) {
            (0, 0, 0) => "".to_string(),
            (0, _, _) => format!("({ty} #{a})"),
            _ => format!("({ty} r{})", a >> 1),
        }
    }

    fn shift_type_mnemonic(t: u32) -> &'static str {
        match t {
            0 => "lsl",
            1 => "lsr",
            2 => "asr",
            _ => "ror",
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
