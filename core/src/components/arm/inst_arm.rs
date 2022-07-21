// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use bitmatch::bitmatch;

use super::interface::{ArmSystem, SysWrapper};
use crate::{
    components::arm::{
        registers::{Flag::*, Mode},
        Access::*,
        Cpu,
    },
    numutil::{NumExt, U32Ext},
};

pub type ArmHandler<S> = fn(&mut SysWrapper<S>, ArmInst);
pub type ArmLut<S> = [ArmHandler<S>; 256];

impl<S: ArmSystem> SysWrapper<S> {
    pub fn execute_inst_arm(&mut self, inst: u32) {
        if !self.check_arm_cond(inst) {
            return;
        }

        let handler = Self::get_handler_arm(inst);
        handler(self, ArmInst(inst));
    }

    pub fn check_arm_cond(&mut self, inst: u32) -> bool {
        self.cpu().eval_condition(inst.bits(28, 4).u16())
    }

    pub fn get_handler_arm(inst: u32) -> ArmHandler<S> {
        S::ARM_LUT[(inst.us() >> 20) & 0xFF]
    }

    pub(crate) fn arm_b<const BL: bool>(&mut self, inst: ArmInst) {
        let nn = inst.0.i24() * 4; // Step 4
        if BL {
            let lr = self.cpur().pc() - 4;
            self.cpu().set_lr(lr);
        }
        self.set_pc(self.cpur().pc().wrapping_add_signed(nn));
    }

    pub(crate) fn arm_bx(&mut self, inst: ArmInst) {
        let rn = self.reg(inst.reg(0));
        if rn.is_bit(0) {
            self.cpu().set_flag(Thumb, true);
            self.set_pc(rn - 1);
        } else {
            self.set_pc(rn);
        }
    }

    pub(crate) fn arm_swi(&mut self, _inst: ArmInst) {
        self.swi();
    }

    pub(crate) fn arm_alu_mul_psr_reg<const OP: u16, const CPSR: bool>(&mut self, inst: ArmInst) {
        // ALU with register OR mul OR psr OR SWP OR BX OR LDR/STR
        // ARM please... what is this instruction encoding
        if OP == 0b1001 && inst.0.bits(8, 13) == 0xFFF {
            // BX
            self.arm_bx(inst);
        } else if !CPSR && (0x8..=0xB).contains(&OP) {
            // MRS/MSR/SWP/LDRSTR
            let bit_1 = OP.is_bit(1);
            let is_msr = OP.is_bit(0);

            if is_msr {
                let m = inst.reg(0);
                self.msr(self.reg(m), inst.0.is_bit(19), inst.0.is_bit(16), bit_1);
            } else {
                let n = inst.reg(16);
                let d = inst.reg(12);
                if n == 15 {
                    // MRS
                    let psr = if bit_1 {
                        self.cpu().spsr()
                    } else {
                        self.cpu().cpsr
                    };
                    self.set_reg(d, psr.set_bit(4, true));
                } else if inst.0.bits(4, 8) == 0b0000_1001 {
                    // SWP
                    self.arm_swp(inst, n, d, bit_1);
                } else {
                    // STRH/LDRH
                    self.arm_strh_ldr::<OP, CPSR>(inst);
                }
            }
        } else if inst.0.bits(4, 4) == 0b1001 {
            // MUL
            self.arm_mul::<OP, CPSR>(inst);
        } else if inst.0.is_bit(4) && inst.0.is_bit(7) {
            self.arm_strh_ldr::<OP, CPSR>(inst);
        } else {
            // ALU
            let m = inst.reg(0);
            let d = inst.reg(12);
            let n = inst.reg(16);
            let t = inst.0.bits(5, 2);
            let carry = self.cpu().flag(Carry);

            if inst.0.is_bit(4) {
                // Shift by reg
                let a = inst.0.bits(8, 4);
                let rm = self.cpu().reg_pc4(m);
                let second_op = self.shifted_op::<CPSR, false>(rm, t, self.reg(a) & 0xFF);
                self.alu::<OP, CPSR, true>(n, second_op, d, carry);
            } else {
                // Shift by imm
                let a = inst.0.bits(7, 5);
                let rm = self.cpu().reg(m);
                let second_op = self.shifted_op::<CPSR, true>(rm, t, a);
                self.alu::<OP, CPSR, false>(n, second_op, d, carry);
            }
        }
    }

    pub(crate) fn arm_alu_imm<const OP: u16, const CPSR: bool>(&mut self, inst: ArmInst) {
        // ALU with register OR MSR
        if !CPSR && (0x8..=0xB).contains(&OP) {
            // MSR
            let spsr = OP.is_bit(1);
            let m = inst.0.bits(8, 4);
            let imm = Cpu::<S>::ror_s0(inst.0 & 0xFF, m << 1);
            self.msr(imm, inst.0.is_bit(19), inst.0.is_bit(16), spsr);
        } else {
            // ALU with immediate
            let carry = self.cpu().flag(Carry);
            let s = inst.0.bits(8, 4);
            let d = inst.reg(12);
            let n = inst.reg(16);
            let second_op = self.cpu().ror::<CPSR, false>(inst.0 & 0xFF, s << 1);
            self.alu::<OP, CPSR, false>(n, second_op, d, carry);
        }
    }

    #[inline]
    pub(crate) fn arm_swp(&mut self, inst: ArmInst, n: u32, d: u32, byte: bool) {
        let m = inst.reg(0);
        let addr = self.reg(n);
        let mem_value = if byte {
            self.read::<u8>(addr, NonSeq).u32()
        } else {
            self.read_word_ldrswp(addr, NonSeq)
        };
        let reg = self.reg(m);
        if byte {
            self.write::<u8>(addr, reg.u8(), NonSeq);
        } else {
            self.write::<u32>(addr, reg, NonSeq);
        }
        self.set_reg(d, mem_value);
        self.idle_nonseq();
    }

    pub(crate) fn arm_strh_ldr<const OP: u16, const LDR: bool>(&mut self, inst: ArmInst) {
        let pre = OP.is_bit(3);
        let up = OP.is_bit(2);
        let imm = OP.is_bit(1);
        let writeback = !pre || OP.is_bit(0);

        let n = inst.reg(16);
        let d = inst.reg(12);

        let offs = if imm {
            inst.0 & 0xF | (inst.0.bits(8, 4) << 4)
        } else {
            self.cpu().reg(inst.reg(0))
        };
        match inst.0.bits(5, 2) {
            1 => {
                // LDRH/STRH
                self.ldrstr::<true>(!pre, up, 2, writeback, !LDR, n, d, offs);
            }
            2 => {
                // LDRSB
                self.ldrstr::<true>(!pre, up, 1, writeback, !LDR, n, d, offs);
                self.set_reg(d, self.reg(d).u8() as i8 as i32 as u32);
            }
            _ => {
                // LDRSH
                self.ldrstr::<false>(!pre, up, 2, writeback, !LDR, n, d, offs);
                self.set_reg(d, self.reg(d).u16() as i16 as i32 as u32);
            }
        }
    }

    pub(crate) fn arm_stm_ldm<const OP: u16>(&mut self, inst: ArmInst) {
        let ldr = OP.is_bit(0);
        let writeback = OP.is_bit(1);
        let user = OP.is_bit(2);
        let up = OP.is_bit(3);
        let pre = OP.is_bit(4);
        let n = inst.reg(16);
        let regs = inst.0 & 0xFFFF;

        let cpsr = self.cpu().cpsr;
        if user {
            self.cpu().set_mode(Mode::System);
        }

        // TODO mehhh, this entire implementation is terrible
        let mut addr = self.reg(n);
        let initial_addr = addr;
        let regs = (0..=15).filter(|b| regs.is_bit(*b)).collect::<Vec<u16>>();
        let first_reg = *regs.get(0).unwrap_or(&12323);
        let end_offs = regs.len().u32() * 4;
        if !up {
            addr = Self::mod_with_offs(addr, 4, !pre);
            addr -= end_offs;
        }
        let mut kind = NonSeq;
        let mut set_n = false;

        for reg in regs {
            set_n |= reg == n.u16();
            if pre {
                addr += 4;
            }
            if !ldr && reg == n.u16() && reg != first_reg {
                self.set_reg(n, Self::mod_with_offs(initial_addr, end_offs, up));
            }

            if ldr {
                let val = self.read::<u32>(addr, kind);
                self.set_reg(reg.u32(), val);
            } else {
                let val = self.cpur().reg_pc4(reg.u32());
                self.write::<u32>(addr, val, kind);
            }

            kind = Seq;
            if !pre {
                addr += 4;
            }
        }

        if writeback && (!ldr || !set_n) {
            self.set_reg(n, Self::mod_with_offs(initial_addr, end_offs, up));
        }

        if user {
            self.cpu().set_cpsr(cpsr);
        }
        if kind == NonSeq {
            self.on_empty_rlist(n, !ldr, up, pre);
        }
        self.cpu().access_type = NonSeq;
        if ldr {
            // All LDR stall by 1I
            self.add_i_cycles(1);
        }
    }

    pub(crate) fn arm_ldrstr<const OP: u16, const IMM: bool>(&mut self, inst: ArmInst) {
        let ldr = OP.is_bit(0);
        let writeback = OP.is_bit(1);
        let byte = OP.is_bit(2);
        let up = OP.is_bit(3);
        let pre = OP.is_bit(4);
        let n = inst.reg(16);
        let d = inst.reg(12);
        let width = if byte { 1 } else { 4 };

        let offs = if IMM {
            inst.0 & 0x7FF
        } else {
            let m = inst.reg(0);
            let s = inst.0.bits(7, 5);
            let t = inst.0.bits(5, 2);
            self.shifted_op::<false, true>(self.reg(m), t, s)
        };
        self.ldrstr::<false>(!pre, up, width, !pre || writeback, !ldr, n, d, offs);
    }

    pub(crate) fn alu<const OP: u16, const CPSR: bool, const SHIFT_REG: bool>(
        &mut self,
        reg_a: u32,
        b: u32,
        dest: u32,
        carry: bool,
    ) {
        let d = self.cpu().reg(dest);

        let reg_a = if SHIFT_REG {
            self.cpu().reg_pc4(reg_a)
        } else {
            self.reg(reg_a)
        };
        let value = match OP {
            0x0 => self.cpu().and::<CPSR>(reg_a, b),
            0x1 => self.cpu().xor::<CPSR>(reg_a, b),
            0x2 => self.cpu().sub::<CPSR>(reg_a, b),
            0x3 => self.cpu().sub::<CPSR>(b, reg_a),
            0x4 => self.cpu().add::<CPSR>(reg_a, b),
            0x5 => self.cpu().adc::<CPSR>(reg_a, b, carry as u32),
            0x6 => self.cpu().sbc::<CPSR>(reg_a, b, carry as u32),
            0x7 => self.cpu().sbc::<CPSR>(b, reg_a, carry as u32),
            0x8 => {
                // TST
                self.cpu().and::<true>(reg_a, b);
                d
            }
            0x9 => {
                // TEQ
                self.cpu().xor::<true>(reg_a, b);
                d
            }
            0xA => {
                // CMP
                self.cpu().sub::<true>(reg_a, b);
                d
            }
            0xB => {
                // CMN
                self.cpu().add::<true>(reg_a, b);
                d
            }
            0xC => self.cpu().or::<CPSR>(reg_a, b),
            0xD => {
                self.cpu().set_nz::<CPSR>(b);
                b
            } // MOV
            0xE => self.cpu().bit_clear::<CPSR>(reg_a, b),
            _ => self.cpu().not::<CPSR>(b),
        };

        if CPSR
            && dest == 15
            && self.cpu().mode() != Mode::User
            && self.cpu().mode() != Mode::System
        {
            // If S=1, not in user/selftem mode and the dest is the PC, set CPSR to current
            // SPSR, also flush pipeline if switch to Thumb occurred
            let spsr = self.cpur().spsr();
            self.cpu().set_cpsr(spsr);
        }

        if !(0x8..=0xB).contains(&OP) {
            // Only write if needed - 8-B should not
            // since they might set PC when they should not
            self.set_reg(dest, value);
        }
    }

    #[inline]
    pub(crate) fn msr(&mut self, src: u32, flags: bool, ctrl: bool, spsr: bool) {
        let mut dest = if spsr {
            self.cpu().spsr()
        } else {
            self.cpu().cpsr
        };

        if flags {
            dest = (dest & 0x00FF_FFFF) | (src & 0xFF00_0000);
        };
        if ctrl && self.cpu().mode() != Mode::User {
            dest = (dest & 0xFFFF_FF00) | (src & 0xFF);
        };

        if spsr {
            self.cpu().set_spsr(dest);
        } else {
            // Thumb flag may not be changed
            dest = dest.set_bit(5, false);
            self.cpu().set_cpsr(dest);
            Cpu::check_if_interrupt(&mut **self);
        }
    }

    #[inline]
    pub(crate) fn arm_mul<const OP: u16, const CPSR: bool>(&mut self, inst: ArmInst) {
        let rm = self.cpu().reg(inst.reg(0));
        let rs = self.cpu().reg(inst.reg(8));
        let rn = self.cpu().reg(inst.reg(12));

        let a = rm as u64;
        let b = rs as u64;
        let dhi = self.reg(inst.reg(16)) as u64;
        let dlo = rn as u64;

        let out: u64 = match OP {
            0b000 => rm.wrapping_mul(rs) as u64,
            0b001 => {
                let r = rm.wrapping_mul(rs);
                let r = r.wrapping_add(rn);
                self.add_i_cycles(1);
                r as u64
            }
            0b010 => a.wrapping_mul(b).wrapping_add(dhi).wrapping_add(dlo), // UMAAL
            0b100 => {
                // UMULL
                self.add_i_cycles(1);
                a.wrapping_mul(b)
            }
            0b101 => {
                // UMLAL
                self.add_i_cycles(2);
                a.wrapping_mul(b).wrapping_add(dlo | (dhi << 32))
            }
            0b110 => {
                // SMULL
                self.add_i_cycles(1);
                (a as i32 as i64).wrapping_mul(b as i32 as i64) as u64
            }
            _ => {
                // SMLAL
                self.add_i_cycles(2);
                (a as i32 as i64)
                    .wrapping_mul(b as i32 as i64)
                    .wrapping_add((dlo | (dhi << 32)) as i64) as u64
            }
        };

        if OP > 0b001 {
            // Don't set high reg on MUL/MLA
            self.set_reg(inst.reg(16), (out >> 32).u32());
            self.set_reg(inst.reg(12), out.u32());
        } else {
            self.set_reg(inst.reg(16), out.u32());
        }
        if CPSR {
            let neg_bit = if OP > 0b001 { 63 } else { 31 };
            self.cpu().set_flag(Zero, out == 0);
            self.cpu().set_flag(Neg, out.is_bit(neg_bit));
            self.cpu().set_flag(Carry, false);
        }

        // TODO signed might be wrong
        self.mul_wait_cycles(b as u32, OP.is_bit(1));
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ldrstr<const ALIGN: bool>(
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
            (true, 4) => {
                let val = self.cpur().reg_pc4(d);
                self.write::<u32>(addr, val, NonSeq);
            }
            (true, 2) => {
                let val = (self.cpur().reg_pc4(d) & 0xFFFF).u16();
                self.write::<u16>(addr, val, NonSeq);
            }
            (true, _) => {
                let val = (self.cpur().reg_pc4(d) & 0xFF).u8();
                self.write::<u8>(addr, val, NonSeq);
            }
            (false, 4) if ALIGN => {
                let val = self.read::<u32>(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, 4) => {
                let val = self.read_word_ldrswp(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, 2) if ALIGN => {
                let val = self.read::<u16>(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, 2) => {
                let val = self.read_hword_ldrsh(addr, NonSeq);
                self.set_reg(d, val);
            }
            (false, _) => {
                let val = self.read::<u8>(addr, NonSeq).u32();
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

        self.cpu().access_type = NonSeq;
        if !str {
            // All LDR stall by 1I
            self.add_i_cycles(1);
        }
    }

    pub(crate) fn shifted_op<const CPSR: bool, const IMM: bool>(
        &mut self,
        nn: u32,
        op: u32,
        shift_amount: u32,
    ) -> u32 {
        if op + shift_amount == 0 {
            // Special case: no shift
            nn
        } else {
            match op {
                0 => self.cpu().lsl::<CPSR>(nn, shift_amount),
                1 => self.cpu().lsr::<CPSR, IMM>(nn, shift_amount),
                2 => self.cpu().asr::<CPSR, IMM>(nn, shift_amount),
                _ => self.cpu().ror::<CPSR, IMM>(nn, shift_amount),
            }
        }
    }

    pub(crate) fn arm_unknown_opcode(_self: &mut Self, inst: ArmInst) {
        Self::log_unknown_opcode(inst.0);
    }
}

impl<S: ArmSystem> Cpu<S> {
    #[bitmatch]
    #[allow(unused_variables)]
    pub fn get_mnemonic_arm(inst: u32) -> String {
        let co = Cpu::<S>::condition_mnemonic(inst.bits(28, 4).u16());
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
                    (0x8..=0xB, _) => format!("{op}{co} r{n}, #{:X} ({c})", Cpu::<S>::ror_s0(m, s)),
                    (0xD | 0xF, 0) => format!("{op}{co} r{d}, #{:X} ({c})", m),
                    (0xD | 0xF, _) => format!("{op}{co} r{d}, #{:X} ({c})", Cpu::<S>::ror_s0(m, s)),
                    (_, 0) => format!("{op}{co} r{d}, r{n}, #{:X} ({c})", m),
                    _ => format!("{op}{co} r{d}, r{n}, #{:X} ({c})", Cpu::<S>::ror_s0(m, s)),
                }
            }

            _ => format!("{:08X}??", inst),
        }
    }

    pub(crate) fn alu_mnemonic(opt: u32) -> &'static str {
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

    pub(crate) fn shift_mnemonic(r: u32, t: u32, a: u32) -> String {
        let ty = Self::shift_type_mnemonic(t);
        match (r, t, a) {
            (0, 0, 0) => "".to_string(),
            (0, _, _) => format!("({ty} #{a})"),
            _ => format!("({ty} r{})", a >> 1),
        }
    }

    pub(crate) fn shift_type_mnemonic(t: u32) -> &'static str {
        match t {
            0 => "lsl",
            1 => "lsr",
            2 => "asr",
            _ => "ror",
        }
    }
}

#[derive(Copy, Clone)]
pub struct ArmInst(pub u32);

impl ArmInst {
    pub(crate) fn reg(self, idx: u32) -> u32 {
        self.0.bits(idx, 4)
    }
}
