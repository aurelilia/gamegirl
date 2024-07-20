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
};
use crate::{access::*, registers::Flag::*};

impl<S: ArmSystem> SysWrapper<S> {
    // UND
    pub(super) fn thumb_unknown_opcode(&mut self, inst: ThumbInst) {
        self.und_inst(inst);
    }

    // THUMB.1/2
    pub(super) fn thumb_arithmetic<const KIND: Thumb12Op>(&mut self, inst: ThumbInst) {
        use Thumb12Op::*;

        let d = inst.reg(0);
        let s = inst.reg(3);
        let n = inst.imm5();

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
    pub(super) fn thumb_3<const KIND: Thumb3Op>(&mut self, inst: ThumbInst) {
        use Thumb3Op::*;

        let d = inst.reg(8);
        let n = inst.imm8();

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
    pub(super) fn thumb_alu(&mut self, inst: ThumbInst) {
        use Thumb4Op::*;

        let d = inst.reg(0);
        let s = inst.reg(3);
        let o = inst.thumb4();

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
    pub(super) fn thumb_hi_add(&mut self, inst: ThumbInst) {
        let (s, d) = inst.reg16();
        let res = self.reg(d.u32()).wrapping_add(self.reg(s.u32()));
        self.set_reg(d.u32(), res);
    }

    pub(super) fn thumb_hi_cmp(&mut self, inst: ThumbInst) {
        let (s, d) = inst.reg16();
        let rs = self.reg(s.u32());
        let rd = self.reg(d.u32());
        self.cpu().sub::<true>(rd, rs);
    }

    pub(super) fn thumb_hi_mov(&mut self, inst: ThumbInst) {
        let (s, d) = inst.reg16();
        self.set_reg(d.u32(), self.reg(s.u32()));
    }

    pub(super) fn thumb_hi_bx(&mut self, inst: ThumbInst) {
        let (s, d) = inst.reg16();
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
    pub(super) fn thumb_ldr6(&mut self, inst: ThumbInst) {
        let d = inst.reg(8);
        let n = inst.imm8();

        self.cpu().registers[d.us()] =
            self.read_word_ldrswp(self.cpur().adj_pc() + (n.u32() << 2), NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.7/8
    pub(super) fn thumb_ldrstr78<const O: ThumbStrLdrOp>(&mut self, inst: ThumbInst) {
        use ThumbStrLdrOp::*;

        let d = inst.reg(0);
        let rb = self.cpu().low(inst.reg(6));
        let ro = self.cpu().low(inst.reg(3));
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
    pub(super) fn thumb_ldrstr9<const O: ThumbStrLdrOp>(&mut self, inst: ThumbInst) {
        use ThumbStrLdrOp::*;

        let d = inst.reg(0);
        let rb = self.cpu().low(inst.reg(3));
        let rd = self.cpu().low(d);
        let n = inst.imm5();
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
    pub(super) fn thumb_ldrstr10<const STR: bool>(&mut self, inst: ThumbInst) {
        let d = inst.reg(0);
        let n = inst.imm5();

        let rb = self.cpu().low(inst.reg(3));
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
    pub(super) fn thumb_str_sp(&mut self, inst: ThumbInst) {
        let n = inst.imm8();
        let d = inst.reg(8);
        let rd = self.low(d);
        let addr = self.cpur().sp() + (n.u32() << 2);
        self.cpu().access_type = NONSEQ;
        self.write::<u32>(addr, rd, NONSEQ);
    }

    pub(super) fn thumb_ldr_sp(&mut self, inst: ThumbInst) {
        let n = inst.imm8();
        let d = inst.reg(8);
        self.cpu().registers[d.us()] =
            self.read_word_ldrswp(self.cpur().sp() + (n.u32() << 2), NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.12
    pub(super) fn thumb_rel_addr<const SP: bool>(&mut self, inst: ThumbInst) {
        let n = inst.imm8();
        let d = inst.reg(8);
        if SP {
            self.cpu().registers[d.us()] = self.cpu().sp() + (n.u32() << 2);
        } else {
            self.cpu().registers[d.us()] = self.cpu().adj_pc() + (n.u32() << 2);
        }
    }

    // THUMB.13
    pub(super) fn thumb_sp_offs(&mut self, inst: ThumbInst) {
        let n = inst.imm7();
        let sp = self.cpur().sp();
        if inst.is_bit(7) {
            self.cpu().set_sp(sp - (n as u32));
        } else {
            self.cpu().set_sp(sp + (n as u32));
        }
    }

    // THUMB.14
    pub(super) fn thumb_push<const SP: bool>(&mut self, inst: ThumbInst) {
        let mut sp = self.cpu().sp();
        let mut kind = NONSEQ;

        if SP {
            sp -= 4;
            let lr = self.cpur().lr();
            self.write::<u32>(sp, lr, kind);
            kind = SEQ;
        }

        for reg in (0..8).rev() {
            if inst.is_bit(reg) {
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

    pub(super) fn thumb_pop<const PC: bool>(&mut self, inst: ThumbInst) {
        let mut sp = self.cpu().sp();
        let mut kind = NONSEQ;

        for reg in 0..8 {
            if inst.is_bit(reg) {
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
    pub(super) fn thumb_stmia(&mut self, inst: ThumbInst) {
        let b = inst.reg(8);
        let mut kind = NONSEQ;
        let mut base_rlist_addr = None;
        let mut rb = self.low(b);

        for reg in 0..8 {
            if inst.is_bit(reg) {
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

    pub(super) fn thumb_ldmia(&mut self, inst: ThumbInst) {
        let b = inst.reg(8);
        let mut kind = NONSEQ;

        for reg in 0..8 {
            if inst.is_bit(reg) {
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
    pub(super) fn thumb_bcond<const COND: u16>(&mut self, inst: ThumbInst) {
        let condition = self.cpu().eval_condition(COND);
        if condition {
            let nn = (inst.imm8() as i8 as i32) * 2; // Step 2
            let cpu = self.cpu();
            let pc = cpu.pc();
            cpu.is_halted = !cpu.waitloop.on_jump(&cpu.registers, pc, nn);
            self.set_pc(pc.wrapping_add_signed(nn));
        }
    }

    // THUMB.17
    pub(super) fn thumb_swi(&mut self, _inst: ThumbInst) {
        self.swi();
    }

    // THUMB.18
    pub(super) fn thumb_br(&mut self, inst: ThumbInst) {
        let nn = (inst.imm10() as i32) * 2; // Step 2
        let cpu = self.cpu();
        let pc = cpu.pc();
        cpu.is_halted = !cpu.waitloop.on_jump(&cpu.registers, pc, nn);
        self.set_pc(pc.wrapping_add_signed(nn));
    }

    // THUMB.19
    pub(super) fn thumb_set_lr(&mut self, inst: ThumbInst) {
        let lr = self
            .cpur()
            .pc()
            .wrapping_add_signed((inst.imm10() as i32) << 12);
        self.cpu().set_lr(lr);
    }

    pub(super) fn thumb_bl<const THUMB: bool>(&mut self, inst: ThumbInst) {
        let pc = self.cpu().pc();
        self.set_pc(self.cpur().lr().wrapping_add(inst.imm11()));
        self.cpu().set_lr(pc - 1);
        self.cpu().set_flag(Thumb, THUMB);
    }
}
