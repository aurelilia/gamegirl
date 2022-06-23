use crate::{
    gga::{
        cpu::{
            registers::{Context, Flag, Flag::*},
            Access::*,
            Cpu, Exception,
        },
        GameGirlAdv,
    },
    numutil::{NumExt, U32Ext},
};
use bitmatch::bitmatch;

impl GameGirlAdv {
    #[bitmatch]
    pub fn execute_inst_arm(&mut self, inst: u32) {
        if !self.cpu.eval_condition(inst.bits(28, 4).u16()) {
            return;
        }

        #[bitmatch]
        match inst {
            "101_lnnnnnnnnnnnnnnnnnnnnnnnn" => {
                let nn = n.i24() * 4; // Step 4
                if l == 1 {
                    // BL
                    self.cpu.set_lr(self.cpu.pc - 4);
                } // else: B
                self.set_pc(self.cpu.pc.wrapping_add_signed(nn));
            }

            "000100101111111111110001_nnnn" => {
                let rn = self.reg(n);
                if rn.is_bit(0) {
                    self.cpu.set_flag(Thumb, true);
                    self.set_pc(rn - 1);
                } else {
                    self.set_pc(rn);
                }
            }

            "1111_????????????????????????" => {
                Cpu::exception_occurred(self, Exception::Swi);
            }

            "00010_0001111dddd000000000000" => self.set_reg(d, self.cpu.cpsr),
            "00010_1001111dddd000000000000" => self.set_reg(d, self.cpu.spsr()),

            "00010_d10f??c111100000000mmmm" => self.msr(self.cpu.reg(m), f == 1, c == 1, d == 1),
            "00110_d10f??c1111mmmmnnnnnnnn" => {
                let imm = Cpu::ror_s0(n, m << 1);
                self.msr(imm, f == 1, c == 1, d == 1);
            }

            "00010_b00nnnndddd00001001mmmm" => {
                let addr = self.reg(n);
                let mem_value = if b == 1 {
                    self.read_byte(addr, NonSeq).u32()
                } else {
                    self.read_word_ldrswp(addr, NonSeq)
                };
                let reg = self.reg(m);
                if b == 1 {
                    self.write_byte(addr, reg.u8(), NonSeq)
                } else {
                    self.write_word(addr, reg, NonSeq)
                }
                self.set_reg(d, mem_value);
                self.add_wait_cycles(1);
            }

            "000_000lcddddnnnnssss1001mmmm" => {
                // MUL/MLA
                let cpsr = self.cpu.cpsr;
                let mut res = self.cpu.mul(self.cpu.reg(m), self.cpu.reg(s));
                if l == 1 {
                    // MLA
                    res = res.wrapping_add(self.cpu.reg(n));
                    self.cpu.set_zn(res);
                    self.add_wait_cycles(1);
                }

                self.set_reg(d, res);
                if c == 0 {
                    // Restore CPSR if we weren't supposed to set flags
                    self.cpu.cpsr = cpsr;
                }

                // TODO proper stall
                self.add_wait_cycles(1);
            }

            "000_0ooocddddnnnnssss1001mmmm" => {
                // MULL/MLAL
                let cpsr = self.cpu.cpsr;
                let a = self.reg(m) as u64;
                let b = self.reg(s) as u64;
                let dhi = self.reg(d) as u64;
                let dlo = self.reg(n) as u64;

                let out: u64 = match o {
                    0b010 => a.wrapping_mul(b).wrapping_add(dhi).wrapping_add(dlo), // UMAAL
                    0b100 => a.wrapping_mul(b),                                     // UMULL
                    0b101 => a.wrapping_mul(b).wrapping_add(dlo | (dhi << 32)),     // UMLAL
                    0b110 => (a as i32 as i64).wrapping_mul(b as i32 as i64) as u64, // SMULL
                    _ => (a as i32 as i64)
                        .wrapping_mul(b as i32 as i64)
                        .wrapping_add((dlo | (dhi << 32)) as i64) as u64, // SMLAL
                };

                self.cpu.set_flag(Flag::Zero, out == 0);
                self.cpu.set_flag(Flag::Neg, out.is_bit(63));
                self.set_reg(d, (out >> 32).u32());
                self.set_reg(n, out.u32());
                if c == 0 {
                    // Restore CPSR if we weren't supposed to set flags
                    self.cpu.cpsr = cpsr;
                }

                // TODO proper stall
                self.add_wait_cycles(1);
            }

            "100_puswlnnnnrrrrrrrrrrrrrrrr" => {
                // STM/LDM
                let cpsr = self.cpu.cpsr;
                if s == 1 {
                    // Use user mode regs
                    self.cpu.set_context(Context::System);
                }

                // TODO mehhh, this entire implementation is terrible
                let mut addr = self.reg(n);
                let mut regs = (0..=15).filter(|b| r.is_bit(*b)).collect::<Vec<u16>>();
                let first_reg = *regs.get(0).unwrap_or(&12323);
                let end_offs = regs.len().u32() * 4;
                if u == 0 {
                    regs.reverse();
                }
                let mut kind = NonSeq;
                let mut set_n = false;

                for reg in regs {
                    set_n |= reg == n.u16();
                    if p == 1 {
                        addr = Self::mod_with_offs(addr, 4, u == 1);
                    }
                    if l == 0 && reg == n.u16() && reg != first_reg {
                        self.set_reg(n, Self::mod_with_offs(self.reg(n), end_offs, u == 1));
                    }

                    if l == 0 {
                        self.write_word(addr, self.cpu.reg_pc4(reg.u32()), kind);
                    } else {
                        let val = self.read_word(addr, kind);
                        self.set_reg(reg.u32(), val);
                    }

                    kind = Seq;
                    if p == 0 {
                        addr = Self::mod_with_offs(addr, 4, u == 1);
                    }
                }

                if w == 1 && (l == 0 || !set_n) {
                    self.set_reg(n, addr);
                }

                self.cpu.cpsr = cpsr;
                if kind == NonSeq {
                    self.on_empty_rlist(n, l == 0, u == 1, p == 1);
                }
                if l == 1 {
                    // All LDR stall by 1I
                    self.add_wait_cycles(1);
                }
            }

            "01_0pubwlnnnnddddmmmmmmmmmmmm" => {
                // LDR/STR with imm
                let width = if b == 1 { 1 } else { 4 };
                self.ldrstr::<false>(p == 0, u == 1, width, (p == 0) || (w == 1), l == 0, n, d, m);
            }
            "01_1pubwlnnnnddddssssstt0mmmm" => {
                // LDR/STR with reg
                let cpsr = self.cpu.cpsr;
                let offs = self.shifted_op::<true>(self.cpu.reg(m), t, s);
                self.cpu.cpsr = cpsr;
                let width = if b == 1 { 1 } else { 4 };
                self.ldrstr::<false>(
                    p == 0,
                    u == 1,
                    width,
                    (p == 0) || (w == 1),
                    l == 0,
                    n,
                    d,
                    offs,
                );
            }

            "000_pu1wlnnnnddddiiii1011iiii" => {
                // LDRH/STRH with imm
                self.ldrstr::<true>(p == 0, u == 1, 2, (p == 0) || (w == 1), l == 0, n, d, i);
            }
            "000_pu0wlnnnndddd00001011mmmm" => {
                // LDRH/STRH with reg
                self.ldrstr::<true>(
                    p == 0,
                    u == 1,
                    2,
                    (p == 0) || (w == 1),
                    l == 0,
                    n,
                    d,
                    self.cpu.reg(m),
                );
            }

            "000_pu1w1nnnnddddiiii1101iiii" => {
                // LDRSB with imm
                self.ldrstr::<true>(p == 0, u == 1, 1, (p == 0) || (w == 1), false, n, d, i);
                self.set_reg(d, self.reg(d).u8() as i8 as i32 as u32);
            }
            "000_pu0w1nnnndddd00001101mmmm" => {
                // LDRSB with reg
                self.ldrstr::<true>(
                    p == 0,
                    u == 1,
                    1,
                    (p == 0) || (w == 1),
                    false,
                    n,
                    d,
                    self.cpu.reg(m),
                );
                self.set_reg(d, self.reg(d).u8() as i8 as i32 as u32);
            }
            "000_pu1w1nnnnddddiiii1111iiii" => {
                // LDRSH with imm
                self.ldrstr::<false>(p == 0, u == 1, 2, (p == 0) || (w == 1), false, n, d, i);
                self.set_reg(d, self.reg(d).u16() as i16 as i32 as u32);
            }
            "000_pu0w1nnnndddd00001111mmmm" => {
                // LDRSH with reg
                self.ldrstr::<false>(
                    p == 0,
                    u == 1,
                    2,
                    (p == 0) || (w == 1),
                    false,
                    n,
                    d,
                    self.cpu.reg(m),
                );
                self.set_reg(d, self.reg(d).u16() as i16 as i32 as u32);
            }

            "000_oooocnnnnddddaaaaattrmmmm" => {
                // ALU with register
                let cpsr = self.cpu.cpsr;
                if r == 0 {
                    // Shift by imm
                    let rm = self.cpu.reg(m);
                    let second_op = self.shifted_op::<true>(rm, t, a);
                    self.alu::<false>(o, n, second_op, d, c == 1, cpsr);
                } else {
                    // Shift by reg
                    let rm = self.cpu.reg_pc4(m);
                    let second_op = self.shifted_op::<false>(rm, t, self.cpu.reg(a >> 1) & 0xFF);
                    self.alu::<true>(o, n, second_op, d, c == 1, cpsr);
                }
                self.add_wait_cycles(1);
            }

            "001_oooocnnnnddddssssmmmmmmmm" => {
                // ALU with immediate
                let cpsr = self.cpu.cpsr;
                let second_op = self.cpu.ror::<false>(m, s << 1);
                self.alu::<false>(o, n, second_op, d, c == 1, cpsr);
            }

            _ => Self::log_unknown_opcode(inst),
        }
    }

    fn alu<const SHIFT_REG: bool>(
        &mut self,
        op: u32,
        reg_a: u32,
        b: u32,
        dest: u32,
        flags: bool,
        cpsr: u32,
    ) {
        let d = self.cpu.reg(dest);
        let carry = cpsr.is_bit(Carry as u16);

        let reg_a = if SHIFT_REG {
            self.cpu.reg_pc4(reg_a)
        } else {
            self.reg(reg_a)
        };
        let value = match op {
            0x0 => self.cpu.and(reg_a, b),
            0x1 => self.cpu.xor(reg_a, b),
            0x2 => self.cpu.sub(reg_a, b),
            0x3 => self.cpu.sub(b, reg_a),
            0x4 => self.cpu.add(reg_a, b),
            0x5 => self.cpu.adc(reg_a, b, carry as u32),
            0x6 => self.cpu.sbc(reg_a, b, carry as u32),
            0x7 => self.cpu.sbc(b, reg_a, carry as u32),
            0x8 => {
                // TST
                self.cpu.and(reg_a, b);
                d
            }
            0x9 => {
                // TEQ
                self.cpu.xor(reg_a, b);
                d
            }
            0xA => {
                // CMP
                self.cpu.sub(reg_a, b);
                d
            }
            0xB => {
                // CMN
                self.cpu.add(reg_a, b);
                d
            }
            0xC => self.cpu.or(reg_a, b),
            0xD => {
                self.cpu.set_zn(b);
                b
            } // MOV
            0xE => self.cpu.bit_clear(reg_a, b),
            _ => self.cpu.not(b),
        };

        if !flags {
            // Restore CPSR if we weren't supposed to set flags
            self.cpu.cpsr = cpsr;
        } else if dest == 15
            && self.cpu.context() != Context::User
            && self.cpu.context() != Context::System
        {
            // If S=1, not in user/system mode and the dest is the PC, set CPSR to current
            // SPSR, also flush pipeline if switch to Thumb occurred
            self.cpu.cpsr = self.cpu.spsr();
            Cpu::check_if_interrupt(self);
            if self.cpu.flag(Thumb) {
                Cpu::pipeline_stall(self);
            }
        }

        if !(0x8..=0xB).contains(&op) {
            // Only write if needed - 8-B should not
            // since they might set PC when they should not
            self.set_reg(dest, value);
        }
    }

    fn msr(&mut self, src: u32, flags: bool, ctrl: bool, spsr: bool) {
        let mut dest = if spsr { self.cpu.spsr() } else { self.cpu.cpsr };

        if flags {
            dest = (dest & 0x00FFFFFF) | (src & 0xFF000000)
        };
        if ctrl && self.cpu.context() != Context::User {
            dest = (dest & 0xFFFFFF00) | (src & 0xFF)
        };

        if spsr {
            self.cpu.set_spsr(dest);
        } else {
            // Thumb flag may not be changed
            dest = dest.set_bit(5, false);
            self.cpu.cpsr = dest;
            Cpu::check_if_interrupt(self);
        }
    }

    fn ldrstr<const ALIGN: bool>(
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
        let mut addr = self.reg(n);
        if !post {
            addr = Self::mod_with_offs(addr, offs, up);
        }

        match (str, width) {
            (true, 4) => self.write_word(addr, self.cpu.reg_pc4(d), NonSeq),
            (true, 2) => self.write_hword(addr, (self.cpu.reg_pc4(d) & 0xFFFF).u16(), NonSeq),
            (true, _) => self.write_byte(addr, (self.cpu.reg_pc4(d) & 0xFF).u8(), NonSeq),
            (false, 4) if ALIGN => {
                let val = self.read_word(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, 4) => {
                let val = self.read_word_ldrswp(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, 2) if ALIGN => {
                let val = self.read_hword(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, 2) => {
                let val = self.read_hword_ldrsh(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, _) => {
                let val = self.read_byte(addr, NonSeq).u32();
                self.set_reg(d, val);
            }
        }

        if post {
            addr = Self::mod_with_offs(addr, offs, up);
        }
        // Edge case: If n == d on an LDR, writeback does nothing
        if writeback && (str || n != d) {
            self.set_reg(n, addr);
        }

        if !str {
            // All LDR stall by 1I
            self.add_wait_cycles(1);
        }
    }

    fn shifted_op<const IMM: bool>(&mut self, nn: u32, op: u32, shift_amount: u32) -> u32 {
        if op + shift_amount == 0 {
            // Special case: no shift
            nn
        } else {
            match op {
                0 => self.cpu.lsl(nn, shift_amount),
                1 => self.cpu.lsr::<IMM>(nn, shift_amount),
                2 => self.cpu.asr::<IMM>(nn, shift_amount),
                _ => self.cpu.ror::<IMM>(nn, shift_amount),
            }
        }
    }

    #[bitmatch]
    #[allow(unused_variables)]
    pub fn get_mnemonic_arm(inst: u32) -> String {
        let co = Cpu::condition_mnemonic(inst.bits(28, 4).u16());
        #[bitmatch]
        match inst {
            "101_0nnnnnnnnnnnnnnnnnnnnnnnn" => format!("b{co} +0x{:X}", (n.i24() << 2) + 8),
            "101_1nnnnnnnnnnnnnnnnnnnnnnnn" => format!("bl{co} +0x{:X}", (n.i24() << 2) + 8),
            "000100101111111111110001_nnnn" => format!("bx{co} r{n}"),
            "1111_nnnnnnnnnnnnnnnnnnnnnnnn" => format!("swi{co} 0x{:07X}", n),

            "00010_000nnnndddd00001001mmmm" => format!("swp{co} r{d}, r{m}, [r{n}]"),
            "00010_100nnnndddd00001001mmmm" => format!("swpb{co} r{d}, r{m}, [r{n}]"),

            "00010_0001111dddd000000000000" => format!("mrs{co} r{d}, cpsr"),
            "00010_1001111dddd000000000000" => format!("mrs{co} r{d}, spsr"),
            "00010_d10fsxc111100000000mmmm" => format!("msr{co} reg (todo)"),
            "00110_d10fsxc1111mmmmnnnnnnnn" => format!("msr{co} imm (todo)"),

            "000_0000cdddd????ssss1001mmmm" => format!("mul{co} r{d}, r{m}, r{s}, ({c})"),
            "000_0001cddddnnnnssss1001mmmm" => format!("mla{co} r{d}, r{m}, r{s}, r{n} ({c})"),
            "000_0010cddddnnnnssss1001mmmm" => {
                format!("umaal{co} r{d}r{n}, (r{m} * r{s} + r{d} + r{n}) ({c})")
            }
            "000_0100cddddnnnnssss1001mmmm" => format!("umull{co} r{d}r{n}, (r{m} * r{s}) ({c})"),
            "000_0101cddddnnnnssss1001mmmm" => {
                format!("umlal{co} r{d}r{n}, (r{m} * r{s} + r{d}r{n}) ({c})")
            }
            "000_0110cddddnnnnssss1001mmmm" => format!("smull{co} r{d}r{n}, (r{m} * r{s}) ({c})"),
            "000_0111cddddnnnnssss1001mmmm" => {
                format!("smlal{co} r{d}r{n}, (r{m} * r{s} + r{d}r{n}) ({c})")
            }

            "100_11??0nnnnrrrrrrrrrrrrrrrr" => format!("stmib r{n}!, {:016b}", r),
            "100_01??0nnnnrrrrrrrrrrrrrrrr" => format!("stmia r{n}!, {:016b}", r),
            "100_10??0nnnnrrrrrrrrrrrrrrrr" => format!("stmdb r{n}!, {:016b}", r),
            "100_00??0nnnnrrrrrrrrrrrrrrrr" => format!("stmda r{n}!, {:016b}", r),
            "100_11??1nnnnrrrrrrrrrrrrrrrr" => format!("ldmib r{n}!, {:016b}", r),
            "100_01??1nnnnrrrrrrrrrrrrrrrr" => format!("ldmia r{n}!, {:016b}", r),
            "100_10??1nnnnrrrrrrrrrrrrrrrr" => format!("ldmdb r{n}!, {:016b}", r),
            "100_00??1nnnnrrrrrrrrrrrrrrrr" => format!("ldmda r{n}!, {:016b}", r),

            "01_0pubwlnnnnddddmmmmmmmmmmmm" => {
                let u = if u == 1 { "+" } else { "-" };
                let b = if b == 1 { "b" } else { "" };
                let op = if l == 1 { "ldr" } else { "str" };
                if p == 1 {
                    format!("{op}{b}{co} r{d}, [r{n}{u}0x{:X}]", m)
                } else {
                    format!("{op}{b}{co} r{d}, [r{n}], {u}0x{:X}", m)
                }
            }
            "01_1pubwlnnnnddddssssstt0mmmm" => {
                let shift = Self::shift_type_mnemonic(t);
                let u = if u == 1 { "+" } else { "-" };
                let b = if b == 1 { "b" } else { "" };
                let op = if l == 1 { "ldr" } else { "str" };
                if p == 1 {
                    format!("{op}{b}{co} r{d}, [r{n}{u}(r{m} {shift} {s})]")
                } else {
                    format!("{op}{b}{co} r{d}, [r{n}], {u}(r{m} {shift} {s})")
                }
            }

            "000_pu1?lnnnnddddiiii1oo1iiii" => {
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
                    format!("{op}{co} r{d}, [r{n} {u}0x{:X}]", i)
                } else {
                    format!("{op}{co} r{d}, [r{n}], {u}0x{:X}", i)
                }
            }
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

            "000_oooocnnnnddddaaaaattrmmmm" => {
                let shift = Self::shift_mnemonic(r, t, a);
                let op = Self::alu_mnemonic(o);
                match o {
                    0x8..=0xB => format!("{op}{co} r{n}, r{m} {shift} ({c})"),
                    0xD | 0xF => format!("{op}{co} r{d}, r{m} {shift} ({c})"),
                    _ => format!("{op}{co} r{d}, r{n}, r{m} {shift} ({c})"),
                }
            }
            "001_oooocnnnnddddssssmmmmmmmm" => {
                let op = Self::alu_mnemonic(o);
                match (o, s) {
                    (0x8..=0xB, 0) => format!("{op}{co} r{n}, #{:X} ({c})", m),
                    (0x8..=0xB, _) => format!("{op}{co} r{n}, #{:X} ({c})", Cpu::ror_s0(m, s)),
                    (0xD | 0xF, 0) => format!("{op}{co} r{d}, #{:X} ({c})", m),
                    (0xD | 0xF, _) => format!("{op}{co} r{d}, #{:X} ({c})", Cpu::ror_s0(m, s)),
                    (_, 0) => format!("{op}{co} r{d}, r{n}, #{:X} ({c})", m),
                    _ => format!("{op}{co} r{d}, r{n}, #{:X} ({c})", Cpu::ror_s0(m, s)),
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
}
