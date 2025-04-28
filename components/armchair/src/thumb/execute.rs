// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla pub(super)lic License Version 2.0 (MPL-2.0) or the
// GNU General pub(super)lic License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::NumExt;

use super::{decode::*, ThumbHandler, ThumbVisitor};
use crate::{
    interface::{Bus, CpuVersion},
    memory::{access::*, Address, RelativeOffset},
    state::{Flag::*, LowRegister, Register},
    Cpu,
};

impl<S: Bus> Cpu<S> {
    pub fn interpret_thumb(&mut self, inst: u16) {
        let handler = Self::get_interpreter_handler_thumb(inst);
        handler(self, ThumbInst::of(inst));
    }

    pub fn get_interpreter_handler_thumb(inst: u16) -> ThumbHandler<Self> {
        S::Version::THUMB.interpreter_lut[inst.us() >> 8]
    }

    pub fn get_cached_handler_thumb(inst: u16) -> ThumbHandler<Self> {
        (S::Version::THUMB.cache_handler_lookup)(ThumbInst::of(inst))
    }
}

impl<S: Bus> ThumbVisitor for Cpu<S> {
    // UND
    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) {
        self.und_inst(inst);
    }

    // THUMB.1/2
    fn thumb_alu_imm(&mut self, kind: Thumb1Op, d: LowRegister, s: LowRegister, n: u32) {
        use Thumb1Op::*;
        let rs = self.state[s];
        let value = match kind {
            Lsl => self.lsl(true, rs, n),
            Lsr => self.lsr::<true>(true, rs, n),
            Asr => self.asr::<true>(true, rs, n),
            Add => self.add(true, rs, n & 7),
            Sub => self.sub(true, rs, n & 7),
        };
        self.state[d] = value;
    }

    // THUMB.2
    fn thumb_2_reg(&mut self, kind: Thumb2Op, d: LowRegister, s: LowRegister, n: LowRegister) {
        let rs = self.state[s];
        let rn = self.state[n];
        let value = match kind {
            Thumb2Op::Add => self.add(true, rs, rn),
            Thumb2Op::Sub => self.sub(true, rs, rn),
        };
        self.state[d] = value;
    }

    // THUMB.3
    fn thumb_3(&mut self, kind: Thumb3Op, d: LowRegister, n: u32) {
        use Thumb3Op::*;
        let rd = self.state[d];
        match kind {
            Mov => {
                self.set_nz(true, n);
                self.state[d] = n;
            }
            Cmp => {
                self.sub(true, rd, n);
            }
            Add => self.state[d] = self.add(true, rd, n),
            Sub => self.state[d] = self.sub(true, rd, n),
        };
    }

    // THUMB.4
    fn thumb_alu(&mut self, kind: Thumb4Op, d: LowRegister, s: LowRegister) {
        use Thumb4Op::*;

        let rd = self.state[d];
        let rs = self.state[s];

        self.state[d] = match kind {
            And => self.and(true, rd, rs),
            Eor => self.xor(true, rd, rs),
            Lsl => {
                self.idle_nonseq();
                self.lsl(true, rd, rs & 0xFF)
            }
            Lsr => {
                self.idle_nonseq();
                self.lsr::<false>(true, rd, rs & 0xFF)
            }
            Asr => {
                self.idle_nonseq();
                self.asr::<false>(true, rd, rs & 0xFF)
            }
            Adc => {
                let c = self.state.is_flag(Carry) as u32;
                self.adc(true, rd, rs, c)
            }
            Sbc => {
                let c = self.state.is_flag(Carry) as u32;
                self.sbc(true, rd, rs, c)
            }
            Ror => {
                self.idle_nonseq();
                self.ror::<false>(true, rd, rs & 0xFF)
            }
            Tst => {
                self.and(true, rd, rs);
                rd
            }
            Neg => self.neg(true, rs),
            Cmp => {
                self.sub(true, rd, rs);
                rd
            }
            Cmn => {
                self.add(true, rd, rs);
                rd
            }
            Orr => self.or(true, rd, rs),
            Mul => {
                self.apply_mul_idle_ticks(rd, true);
                self.mul(true, rd, rs)
            }
            Bic => self.bit_clear(true, rd, rs),
            Mvn => self.not(true, rs),
        }
    }

    // THUMB.5
    fn thumb_hi_add(&mut self, (s, d): (Register, Register)) {
        let res = self.state[d].wrapping_add(self.state[s]);
        self.set_reg(d, res);
    }

    fn thumb_hi_cmp(&mut self, (s, d): (Register, Register)) {
        let rs = self.state[s];
        let rd = self.state[d];
        self.sub(true, rd, rs);
    }

    fn thumb_hi_mov(&mut self, (s, d): (Register, Register)) {
        self.set_reg(d, self.state[s]);
    }

    fn thumb_hi_bx(&mut self, s: Register, blx: bool) {
        if blx {
            // BLX
            let rn = self.state[s];
            // Is this v5 behavior correct?
            if S::Version::IS_V5 {
                let pc = self.state.pc() - Address(1);
                self.state.set_lr(pc);
                if !rn.is_bit(0) {
                    self.state.set_flag(Thumb, false);
                }
            }
            self.set_pc(Address(rn));
        } else if s.is_pc() {
            // BX ARM switch
            self.state.set_flag(Thumb, false);
            self.set_pc(self.state.pc()); // Align
        } else {
            // BX
            if self.state[s].is_bit(0) {
                self.set_pc(Address(self.state[s] & !1));
            } else {
                self.state.set_flag(Thumb, false);
                self.set_pc(Address(self.state[s] & !3));
            }
        }
    }

    // THUMB.6
    fn thumb_ldr6(&mut self, d: LowRegister, offset: Address) {
        self.state[d] = self.read_word_ldrswp(self.state.adj_pc() + offset, NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.7/8
    fn thumb_ldrstr78(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        o: LowRegister,
    ) {
        use ThumbStrLdrOp::*;

        let rb = self.state[b];
        let ro = self.state[o];
        let rd = self.state[d];
        let addr = Address(rb.wrapping_add(ro));
        self.state.access_type = NONSEQ;

        match op {
            Str => self.write::<u32>(addr, rd, NONSEQ),
            Strh => self.write::<u16>(addr, rd.u16(), NONSEQ),
            Strb => self.write::<u8>(addr, rd.u8(), NONSEQ),
            Ldsb => self.state[d] = self.read::<u8>(addr, NONSEQ) as i8 as i32 as u32,
            Ldr => self.state[d] = self.read_word_ldrswp(addr, NONSEQ),
            Ldrh => self.state[d] = self.read::<u16>(addr, NONSEQ),
            Ldrb => self.state[d] = self.read::<u8>(addr, NONSEQ).u32(),
            // LDSH, needs special handling for unaligned reads which makes it behave as LDRSB
            Ldsh if addr.0.is_bit(0) => {
                self.state[d] = self.read::<u8>(addr, NONSEQ) as i8 as i32 as u32;
            }
            Ldsh => self.state[d] = self.read::<u16>(addr, NONSEQ) as i16 as i32 as u32,
        }
        if op as usize > 2 {
            // LDR has +1I
            self.bus.tick(1);
        }
    }

    // THUMB.9
    fn thumb_ldrstr9(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) {
        use ThumbStrLdrOp::*;

        let rb = Address(self.state[b]);
        let rd = self.state[d];
        self.state.access_type = NONSEQ;

        match op {
            Str => self.write::<u32>(rb + offset, rd, NONSEQ),
            Strb => self.write::<u8>(rb + offset, rd.u8(), NONSEQ),

            Ldr => self.state[d] = self.read_word_ldrswp(rb + offset, NONSEQ),
            Ldrb => self.state[d] = self.read::<u8>(rb + offset, NONSEQ).u32(),

            _ => unreachable!(),
        }

        if op == Ldr || op == Ldrb {
            // LDR has +1I
            self.bus.tick(1);
        }
    }

    // THUMB.10
    fn thumb_ldrstr10(&mut self, str: bool, d: LowRegister, b: LowRegister, offset: Address) {
        let rd = self.state[d];
        let addr = Address(self.state[b]) + offset;
        self.state.access_type = NONSEQ;

        if str {
            self.write::<u16>(addr, rd.u16(), NONSEQ);
        } else {
            self.state[d] = self.read::<u16>(addr, NONSEQ).u32();
            // LDR has +1I
            self.bus.tick(1);
        }
    }

    // THUMB.11
    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address) {
        let rd = self.state[d];
        let addr = self.state.sp() + offset;
        self.state.access_type = NONSEQ;
        self.write::<u32>(addr, rd, NONSEQ);
    }

    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address) {
        self.state[d] = self.read_word_ldrswp(self.state.sp() + offset, NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.12
    fn thumb_rel_addr(&mut self, sp: bool, d: LowRegister, offset: Address) {
        if sp {
            self.state[d] = (self.state.sp() + offset).0;
        } else {
            self.state[d] = (self.state.adj_pc() + offset).0;
        }
    }

    // THUMB.13
    fn thumb_sp_offs(&mut self, offset: RelativeOffset) {
        let sp = self.state.sp();
        self.state.set_sp(sp.add_rel(offset));
    }

    // THUMB.14
    fn thumb_push(&mut self, reg_list: u8, lr: bool) {
        let mut sp = self.state.sp();
        let mut kind = NONSEQ;

        if lr {
            sp -= Address::WORD;
            let lr = self.state.lr();
            self.write::<u32>(sp, lr.0, kind);
            kind = SEQ;
        }

        for reg in LowRegister::from_rlist(reg_list).rev() {
            sp -= Address::WORD;
            let reg = self.state[reg];
            self.write::<u32>(sp, reg, kind);
            kind = SEQ;
        }

        assert!(kind == SEQ);
        self.state.set_sp(sp);
        self.state.access_type = NONSEQ;
    }

    fn thumb_pop(&mut self, reg_list: u8, pc: bool) {
        let mut sp = self.state.sp();
        let mut kind = NONSEQ;

        for reg in LowRegister::from_rlist(reg_list) {
            self.state[reg] = self.read::<u32>(sp, kind);
            sp += Address::WORD;
            kind = SEQ;
        }

        if pc {
            let pc = self.read::<u32>(sp, kind);
            if S::Version::IS_V5 && !pc.is_bit(0) {
                self.state.set_flag(Thumb, false);
            }
            self.set_pc(Address(pc));
            sp += Address::WORD;
            kind = SEQ;
        }

        assert!(kind == SEQ);
        self.state.set_sp(sp);
        self.idle_nonseq();
    }

    // THUMB.15
    fn thumb_stmia(&mut self, b: LowRegister, reg_list: u8) {
        let mut kind = NONSEQ;
        let mut base_rlist_addr = None;
        let mut rb = Address(self.state[b]);

        for reg in LowRegister::from_rlist(reg_list) {
            if reg == b && kind != NONSEQ {
                base_rlist_addr = Some(Address(self.state[b]));
            }
            let reg = self.state[reg];
            self.write::<u32>(rb, reg, kind);
            rb += Address::WORD;
            self.state[b] = rb.0;
            kind = SEQ;
        }

        if let Some(addr) = base_rlist_addr {
            // If base was in Rlist and not the first, write final address to that location.
            // We ignore timing since this was already (wrongly) written in the loop above.
            self.bus.set::<u32>(&mut self.state, addr, rb.0);
        }

        if kind == NONSEQ {
            self.on_empty_rlist(Register(b.0), true, true, false);
        }
        self.state.access_type = NONSEQ;
    }

    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8) {
        let mut kind = NONSEQ;

        for reg in LowRegister::from_rlist(reg_list) {
            let addr = self.state[b];
            self.state[reg] = self.read::<u32>(Address(addr), kind);
            self.state[b] = self.state[b].wrapping_add(4);
            kind = SEQ;
        }

        if kind == NONSEQ {
            self.on_empty_rlist(Register(b.0), false, true, false);
        }
        self.idle_nonseq();
    }

    // THUMB.16
    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset) {
        let condition = self.state.eval_condition(cond);
        if condition {
            self.relative_jump(offset);
        }
    }

    // THUMB.17
    fn thumb_swi(&mut self) {
        self.exception_occured(crate::Exception::Swi);
    }

    // THUMB.18
    fn thumb_br(&mut self, offset: RelativeOffset) {
        self.relative_jump(offset);
    }

    // THUMB.19
    fn thumb_set_lr(&mut self, offset: RelativeOffset) {
        let lr = self.state.pc().add_rel(offset);
        self.state.set_lr(lr);
    }

    fn thumb_bl(&mut self, offset: Address, thumb: bool) {
        let pc = self.state.pc();
        self.set_pc(self.state.lr() + offset);
        self.state.set_lr(pc - Address::BYTE);
        self.state.set_flag(Thumb, thumb);
    }
}
