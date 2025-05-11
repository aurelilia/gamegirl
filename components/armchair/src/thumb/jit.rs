// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla pub(super)lic License Version 2.0 (MPL-2.0) or the
// GNU General pub(super)lic License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::NumExt;
use cranelift::prelude::*;

use super::{decode::*, ThumbVisitor};
use crate::{
    interface::Bus,
    memory::{access::*, Address, RelativeOffset},
    optimizations::{
        analyze::InstructionAnalysis,
        jit::{Condition, InstructionTranslator},
    },
    state::{LowRegister, Register},
    Cpu,
};

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn translate_thumb(&mut self, instr: &InstructionAnalysis) {
        let wait = self
            .bus
            .wait_time::<u16>(&mut self.cpu, self.current_instruction, SEQ);
        self.insert_instruction_preamble(wait as u64, self.consts.two_i32, instr.is_branch_target);
        if self.bus.debugger().tracing() {
            let inst = self.imm(instr.instr as i64, types::I32);
            self.trace_inst_thumb(inst);
        }

        let inst = ThumbInst::of(instr.instr.u16());
        let handle = super::decode::get_instruction_handler(inst);
        let implemented = handle(self, inst);
        if !implemented {
            let inst = self.imm(instr.instr as i64, types::I16);
            self.interpret_thumb(inst);
        }
        self.stats.total_instructions += 1;
        self.stats.native_instructions += implemented as usize;
    }

    fn load_adj_pc(&mut self) -> Value {
        let pc = self.load_pc();
        self.builder.ins().band_imm(pc, !2)
    }
}

impl<S: Bus> ThumbVisitor for InstructionTranslator<'_, '_, '_, S> {
    type Output = bool;

    fn thumb_unknown_opcode(&mut self, _inst: ThumbInst) -> Self::Output {
        false
    }

    fn thumb_alu_imm(
        &mut self,
        kind: Thumb1Op,
        d: LowRegister,
        s: LowRegister,
        n: u32,
    ) -> Self::Output {
        use Thumb1Op::*;
        let rs = self.load_lreg(s);
        let value = match kind {
            Lsl => self.lsl_imm(true, rs, n),
            Add => self.add_imm(true, rs, n),
            Sub => self.sub_imm(true, rs, n),
            _ => return false,
        };
        self.store_lreg(d, value);
        true
    }

    fn thumb_2_reg(
        &mut self,
        kind: Thumb2Op,
        d: LowRegister,
        s: LowRegister,
        n: LowRegister,
    ) -> Self::Output {
        let rs = self.load_lreg(s);
        let rn = self.load_lreg(n);
        let value = match kind {
            Thumb2Op::Add => self.add(true, rs, rn),
            Thumb2Op::Sub => self.sub(true, rs, rn),
        };
        self.store_lreg(d, value);
        true
    }

    fn thumb_3(&mut self, kind: Thumb3Op, d: LowRegister, n: u32) -> Self::Output {
        use Thumb3Op::*;
        let rd = self.load_lreg(d);
        let nn = self.builder.ins().iconst(types::I32, n as i64);
        match kind {
            Mov => {
                self.set_nz(nn);
                self.store_lreg(d, nn);
            }
            Cmp => {
                self.sub(true, rd, nn);
            }
            Add => {
                let value = self.add(true, rd, nn);
                self.store_lreg(d, value);
            }
            Sub => {
                let value = self.sub(true, rd, nn);
                self.store_lreg(d, value);
            }
        };
        true
    }

    fn thumb_alu(&mut self, kind: Thumb4Op, d: LowRegister, s: LowRegister) -> Self::Output {
        use Thumb4Op::*;
        let rs = self.load_lreg(s);
        let rd = self.load_lreg(d);

        let value = match kind {
            And => self.and(true, rd, rs),
            Eor => self.xor(true, rd, rs),
            Lsl => {
                // TODO tick
                let rs = self.builder.ins().band_imm(rs, 0xFF);
                self.lsl(true, rd, rs)
            }
            Tst => {
                self.and(true, rd, rs);
                return true;
            }
            Neg => self.neg(true, rs),
            Cmp => {
                self.sub(true, rd, rs);
                return true;
            }
            Cmn => {
                self.add(true, rd, rs);
                return true;
            }
            Orr => self.or(true, rd, rs),
            Mul => {
                // TODO tick
                self.mul(true, rd, rs)
            }
            Bic => self.bit_clear(true, rd, rs),
            Mvn => self.not(true, rs),
            _ => return false,
        };

        self.store_lreg(d, value);
        true
    }

    fn thumb_hi_add(&mut self, (s, d): (Register, Register)) -> Self::Output {
        let rd = self.load_reg(d);
        let rs = self.load_reg(s);
        let out = self.builder.ins().iadd(rd, rs);
        self.store_reg(d, out);
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        true
    }

    fn thumb_hi_cmp(&mut self, (s, d): (Register, Register)) -> Self::Output {
        let rs = self.load_reg(s);
        let rd = self.load_reg(d);
        self.sub(true, rd, rs);
        true
    }

    fn thumb_hi_mov(&mut self, (s, d): (Register, Register)) -> Self::Output {
        let rs = self.load_reg(s);
        self.store_reg(d, rs);
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        true
    }

    fn thumb_hi_bx(&mut self, s: Register, blx: bool) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn thumb_ldr6(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        false
    }

    fn thumb_ldrstr78(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        o: LowRegister,
    ) -> Self::Output {
        use ThumbStrLdrOp::*;
        if matches!(op, Str | Strb | Strh) {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn thumb_ldrstr9(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        use ThumbStrLdrOp::*;
        if matches!(op, Str | Strb | Strh) {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn thumb_ldrstr10(
        &mut self,
        str: bool,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        if str {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        false
    }

    fn thumb_rel_addr(&mut self, sp: bool, d: LowRegister, offset: Address) -> Self::Output {
        let reg = if sp {
            self.load_sp()
        } else {
            self.load_adj_pc()
        };
        let value = self.builder.ins().iadd_imm(reg, offset.0 as i64);
        self.store_lreg(d, value);
        true
    }

    fn thumb_sp_offs(&mut self, offset: RelativeOffset) -> Self::Output {
        let sp = self.load_sp();
        let value = self.builder.ins().iadd_imm(sp, offset.0 as i64);
        self.store_sp(value);
        true
    }

    fn thumb_push(&mut self, reg_list: u8, lr: bool) -> Self::Output {
        false
    }

    fn thumb_pop(&mut self, reg_list: u8, pc: bool) -> Self::Output {
        if pc {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn thumb_stmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output {
        false
    }

    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset) -> Self::Output {
        let cond = self.evaluate_condition(cond);
        match cond {
            Condition::RunAlways => self.thumb_br(offset),
            Condition::RunNever => true,

            Condition::RunIf(value) => {
                let exec_block = self.builder.create_block();
                let cont_block = self.builder.create_block();
                self.builder
                    .ins()
                    .brif(value, exec_block, &[], cont_block, &[]);

                self.builder.switch_to_block(exec_block);
                self.relative_jump(offset);
                self.builder.seal_block(exec_block);

                self.builder.switch_to_block(cont_block);
                true
            }
        }
    }

    fn thumb_swi(&mut self) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn thumb_br(&mut self, offset: RelativeOffset) -> Self::Output {
        self.relative_jump(offset);
        let next = self.builder.create_block();
        self.builder.switch_to_block(next);
        true
    }

    fn thumb_set_lr(&mut self, offset: RelativeOffset) -> Self::Output {
        let pc = self.load_pc();
        let value = self.builder.ins().iadd_imm(pc, offset.0 as i64);
        self.store_lr(value);
        true
    }

    fn thumb_bl(&mut self, offset: Address, thumb: bool) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }
}
