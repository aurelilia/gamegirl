// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla pub(super)lic License Version 2.0 (MPL-2.0) or the
// GNU General pub(super)lic License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::NumExt;

use super::{
    super::interface::{ArmSystem, SysWrapper},
    decode::*,
    ThumbExecutor,
};
use crate::{access::*, registers::Flag::*};

impl<S: ArmSystem> ThumbExecutor for SysWrapper<S> {
    // UND
    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) {
        self.und_inst(inst);
    }

    // THUMB.1/2
    fn thumb_arithmetic<const KIND: Thumb12Op>(&mut self, d: u16, s: u16, n: u16) {
        use Thumb12Op::*;

        let rs = self.low(s);
        let value = match KIND {
            Lsl => self.cpu().lsl::<true>(rs, n.u32()),
            Lsr => self.cpu().lsr::<true, true>(rs, n.u32()),
            Asr => self.cpu().asr::<true, true>(rs, n.u32()),

            AddReg => {
                let rn = self.low(n & 7);
                self.cpu().add::<true>(rs, rn)
            }
            SubReg => {
                let rn = self.low(n & 7);
                self.cpu().sub::<true>(rs, rn)
            }

            AddImm => self.cpu().add::<true>(rs, (n & 7).u32()),
            SubImm => self.cpu().sub::<true>(rs, (n & 7).u32()),
        };
        self.cpu().registers[d.us()] = value;
    }

    // THUMB.3
    fn thumb_3<const KIND: Thumb3Op>(&mut self, d: u16, n: u16) {
        use Thumb3Op::*;

        let rd = self.low(d);
        match KIND {
            Mov => {
                self.cpu().set_nz::<true>(n.u32());
                self.cpu().registers[d.us()] = n.u32();
            }
            Cmp => {
                self.cpu().sub::<true>(rd, n.u32());
            }
            Add => self.cpu().registers[d.us()] = self.cpu().add::<true>(rd, n.u32()),
            Sub => self.cpu().registers[d.us()] = self.cpu().sub::<true>(rd, n.u32()),
        };
    }

    // THUMB.4
    fn thumb_alu(&mut self, o: Thumb4Op, d: u16, s: u16) {
        use Thumb4Op::*;

        let rd = self.low(d);
        let rs = self.low(s);

        self.cpu().registers[d.us()] = match o {
            And => self.cpu().and::<true>(rd, rs),
            Eor => self.cpu().xor::<true>(rd, rs),
            Lsl => {
                self.idle_nonseq();
                self.cpu().lsl::<true>(rd, rs & 0xFF)
            }
            Lsr => {
                self.idle_nonseq();
                self.cpu().lsr::<true, false>(rd, rs & 0xFF)
            }
            Asr => {
                self.idle_nonseq();
                self.cpu().asr::<true, false>(rd, rs & 0xFF)
            }
            Adc => {
                let c = self.cpur().flag(Carry) as u32;
                self.cpu().adc::<true>(rd, rs, c)
            }
            Sbc => {
                let c = self.cpur().flag(Carry) as u32;
                self.cpu().sbc::<true>(rd, rs, c)
            }
            Ror => {
                self.idle_nonseq();
                self.cpu().ror::<true, false>(rd, rs & 0xFF)
            }
            Tst => {
                self.cpu().and::<true>(rd, rs);
                rd
            }
            Neg => self.cpu().neg::<true>(rs),
            Cmp => {
                self.cpu().sub::<true>(rd, rs);
                rd
            }
            Cmn => {
                self.cpu().add::<true>(rd, rs);
                rd
            }
            Orr => self.cpu().or::<true>(rd, rs),
            Mul => {
                self.mul_wait_cycles(rd, true);
                self.cpu().mul::<true>(rd, rs)
            }
            Bic => self.cpu().bit_clear::<true>(rd, rs),
            Mvn => self.cpu().not::<true>(rs),
        }
    }

    // THUMB.5
    fn thumb_hi_add(&mut self, (s, d): (u16, u16)) {
        let res = self.reg(d.u32()).wrapping_add(self.reg(s.u32()));
        self.set_reg(d.u32(), res);
    }

    fn thumb_hi_cmp(&mut self, (s, d): (u16, u16)) {
        let rs = self.reg(s.u32());
        let rd = self.reg(d.u32());
        self.cpu().sub::<true>(rd, rs);
    }

    fn thumb_hi_mov(&mut self, (s, d): (u16, u16)) {
        self.set_reg(d.u32(), self.reg(s.u32()));
    }

    fn thumb_hi_bx(&mut self, (s, d): (u16, u16)) {
        if d > 7 {
            // BLX
            let rn = self.reg(s.u32());
            // Is this v5 behavior correct?
            if S::IS_V5 {
                let pc = self.cpur().pc() - 1;
                self.cpu().set_lr(pc);
                if !rn.is_bit(0) {
                    self.cpu().set_flag(Thumb, false);
                }
            }
            self.set_pc(rn);
        } else if s == 15 {
            // BX ARM switch
            self.cpu().set_flag(Thumb, false);
            self.set_pc(self.cpur().pc()); // Align
        } else {
            // BX
            if self.reg(s.u32()).is_bit(0) {
                self.set_pc(self.reg(s.u32()) & !1);
            } else {
                self.cpu().set_flag(Thumb, false);
                self.set_pc(self.reg(s.u32()) & !3);
            }
        }
    }

    // THUMB.6
    fn thumb_ldr6(&mut self, d: u16, n: u16) {
        self.cpu().registers[d.us()] =
            self.read_word_ldrswp(self.cpur().adj_pc() + (n.u32() << 2), NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.7/8
    fn thumb_ldrstr78<const O: ThumbStrLdrOp>(&mut self, d: u16, b: u16, o: u16) {
        use ThumbStrLdrOp::*;

        let rb = self.cpu().low(b);
        let ro = self.cpu().low(o);
        let rd = self.cpu().low(d);
        let addr = rb.wrapping_add(ro);
        self.cpu().access_type = NONSEQ;

        match O {
            Str => self.write::<u32>(addr, rd, NONSEQ),
            Strh => self.write::<u16>(addr, rd.u16(), NONSEQ),
            Strb => self.write::<u8>(addr, rd.u8(), NONSEQ),
            Ldsb => {
                self.cpu().registers[d.us()] = self.read::<u8>(addr, NONSEQ) as i8 as i32 as u32
            }
            Ldr => self.cpu().registers[d.us()] = self.read_word_ldrswp(addr, NONSEQ),
            Ldrh => self.cpu().registers[d.us()] = self.read::<u16>(addr, NONSEQ),
            Ldrb => self.cpu().registers[d.us()] = self.read::<u8>(addr, NONSEQ).u32(),
            // LDSH, needs special handling for unaligned reads which makes it behave as LDRSB
            Ldsh if addr.is_bit(0) => {
                self.cpu().registers[d.us()] = self.read::<u8>(addr, NONSEQ) as i8 as i32 as u32;
            }
            Ldsh => {
                self.cpu().registers[d.us()] = self.read::<u16>(addr, NONSEQ) as i16 as i32 as u32
            }
        }
        if O as usize > 2 {
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.9
    fn thumb_ldrstr9<const O: ThumbStrLdrOp>(&mut self, d: u16, b: u16, n: u16) {
        use ThumbStrLdrOp::*;

        let rb = self.cpu().low(b);
        let rd = self.cpu().low(d);
        self.cpu().access_type = NONSEQ;

        match O {
            Str => self.write::<u32>(rb + (n.u32() << 2), rd, NONSEQ),
            Ldr => {
                self.cpu().registers[d.us()] = self.read_word_ldrswp(rb + (n.u32() << 2), NONSEQ)
            }
            Strb => self.write::<u8>(rb + n.u32(), rd.u8(), NONSEQ),
            Ldrb => self.cpu().registers[d.us()] = self.read::<u8>(rb + n.u32(), NONSEQ).u32(),
            _ => unreachable!(),
        }

        if O == Ldr || O == Ldrb {
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.10
    fn thumb_ldrstr10<const STR: bool>(&mut self, d: u16, b: u16, n: u16) {
        let rb = self.cpu().low(b);
        let ro = n.u32() << 1; // Step 2
        let rd = self.cpu().low(d);
        let addr = rb + ro;
        self.cpu().access_type = NONSEQ;

        if STR {
            self.write::<u16>(addr, rd.u16(), NONSEQ);
        } else {
            self.cpu().registers[d.us()] = self.read::<u16>(addr, NONSEQ).u32();
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.11
    fn thumb_str_sp(&mut self, d: u16, n: u16) {
        let rd = self.low(d);
        let addr = self.cpur().sp() + (n.u32() << 2);
        self.cpu().access_type = NONSEQ;
        self.write::<u32>(addr, rd, NONSEQ);
    }

    fn thumb_ldr_sp(&mut self, d: u16, n: u16) {
        self.cpu().registers[d.us()] =
            self.read_word_ldrswp(self.cpur().sp() + (n.u32() << 2), NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.12
    fn thumb_rel_addr<const SP: bool>(&mut self, d: u16, n: u16) {
        if SP {
            self.cpu().registers[d.us()] = self.cpu().sp() + (n.u32() << 2);
        } else {
            self.cpu().registers[d.us()] = self.cpu().adj_pc() + (n.u32() << 2);
        }
    }

    // THUMB.13
    fn thumb_sp_offs(&mut self, n: u16, offset_neg: bool) {
        let sp = self.cpur().sp();
        if offset_neg {
            self.cpu().set_sp(sp - (n as u32));
        } else {
            self.cpu().set_sp(sp + (n as u32));
        }
    }

    // THUMB.14
    fn thumb_push<const SP: bool>(&mut self, reg_list: u16) {
        let mut sp = self.cpu().sp();
        let mut kind = NONSEQ;

        if SP {
            sp -= 4;
            let lr = self.cpur().lr();
            self.write::<u32>(sp, lr, kind);
            kind = SEQ;
        }

        for reg in (0..8).rev() {
            if reg_list.is_bit(reg) {
                sp -= 4;
                let reg = self.cpur().registers[reg.us()];
                self.write::<u32>(sp, reg, kind);
                kind = SEQ;
            }
        }

        assert!(kind == SEQ);
        self.cpu().set_sp(sp);
        self.cpu().access_type = NONSEQ;
    }

    fn thumb_pop<const PC: bool>(&mut self, reg_list: u16) {
        let mut sp = self.cpu().sp();
        let mut kind = NONSEQ;

        for reg in 0..8 {
            if reg_list.is_bit(reg) {
                self.cpu().registers[reg.us()] = self.read::<u32>(sp, kind);
                sp += 4;
                kind = SEQ;
            }
        }

        if PC {
            let pc = self.read::<u32>(sp, kind);
            if S::IS_V5 && !pc.is_bit(0) {
                self.cpu().set_flag(Thumb, false);
            }
            self.set_pc(pc);
            sp += 4;
            kind = SEQ;
        }

        assert!(kind == SEQ);
        self.cpu().set_sp(sp);
        self.idle_nonseq();
    }

    // THUMB.15
    fn thumb_stmia(&mut self, b: u16, reg_list: u16) {
        let mut kind = NONSEQ;
        let mut base_rlist_addr = None;
        let mut rb = self.low(b);

        for reg in 0..8 {
            if reg_list.is_bit(reg) {
                if reg == b && kind != NONSEQ {
                    base_rlist_addr = Some(self.low(b));
                }
                let reg = self.low(reg);
                self.write::<u32>(rb, reg, kind);
                rb = rb.wrapping_add(4);
                self.cpu().registers[b.us()] = rb;
                kind = SEQ;
            }
        }

        if let Some(addr) = base_rlist_addr {
            // If base was in Rlist and not the first, write final address to that location.
            // We ignore timing since this was already (wrongly) written in the loop above.
            self.set::<u32>(addr, rb);
        }

        if kind == NONSEQ {
            self.on_empty_rlist(b.u32(), true, true, false);
        }
        self.cpu().access_type = NONSEQ;
    }

    fn thumb_ldmia(&mut self, b: u16, reg_list: u16) {
        let mut kind = NONSEQ;

        for reg in 0..8 {
            if reg_list.is_bit(reg) {
                let addr = self.low(b);
                self.cpu().registers[reg.us()] = self.read::<u32>(addr, kind);
                self.cpu().registers[b.us()] = self.low(b).wrapping_add(4);
                kind = SEQ;
            }
        }

        if kind == NONSEQ {
            self.on_empty_rlist(b.u32(), false, true, false);
        }
        self.idle_nonseq();
    }

    // THUMB.16
    fn thumb_bcond<const COND: u16>(&mut self, n: u16) {
        let condition = self.cpu().eval_condition(COND);
        if condition {
            let nn = (n as i8 as i32) * 2; // Step 2
            let cpu = self.cpu();
            let pc = cpu.pc();
            cpu.is_halted = !cpu.waitloop.on_jump(&cpu.registers, pc, nn);
            self.set_pc(pc.wrapping_add_signed(nn));
        }
    }

    // THUMB.17
    fn thumb_swi(&mut self) {
        self.swi();
    }

    // THUMB.18
    fn thumb_br(&mut self, n: i16) {
        let nn = (n as i32) * 2; // Step 2
        let cpu = self.cpu();
        let pc = cpu.pc();
        cpu.is_halted = !cpu.waitloop.on_jump(&cpu.registers, pc, nn);
        self.set_pc(pc.wrapping_add_signed(nn));
    }

    // THUMB.19
    fn thumb_set_lr(&mut self, n: i16) {
        let lr = self.cpur().pc().wrapping_add_signed((n as i32) << 12);
        self.cpu().set_lr(lr);
    }

    fn thumb_bl<const THUMB: bool>(&mut self, n: u32) {
        let pc = self.cpu().pc();
        self.set_pc(self.cpur().lr().wrapping_add(n));
        self.cpu().set_lr(pc - 1);
        self.cpu().set_flag(Thumb, THUMB);
    }
}
