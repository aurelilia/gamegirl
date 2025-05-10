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
    optimizations::{analyze::InstructionAnalysis, jit::InstructionTranslator},
    state::{LowRegister, Register},
    Cpu,
};

const TRACE: bool = true;

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn translate_thumb(&mut self, addr: Address, instr: &InstructionAnalysis) {
        let wait = self.bus.wait_time::<u16>(&mut self.cpu, addr, SEQ);
        self.insert_instruction_preamble(wait as u64, self.consts.two_i32);
        if TRACE {
            let inst = self.imm(instr.instr as i64, types::I32);
            self.call_cpui32(Cpu::<S>::trace_inst::<u16>, inst);
        }

        let inst = ThumbInst::of(instr.instr.u16());
        let handle = super::decode::get_instruction_handler(inst);
        let implemented = handle(self, inst);
        if !implemented {
            let inst = self.imm(instr.instr as i64, types::I16);
            self.call_cpui16(Cpu::<S>::interpret_thumb, inst);
        }
    }
}

impl<S: Bus> ThumbVisitor for InstructionTranslator<'_, '_, '_, S> {
    type Output = bool;

    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) -> Self::Output {
        false
    }

    fn thumb_alu_imm(
        &mut self,
        kind: Thumb1Op,
        d: LowRegister,
        s: LowRegister,
        n: u32,
    ) -> Self::Output {
        false
    }

    fn thumb_2_reg(
        &mut self,
        kind: Thumb2Op,
        d: LowRegister,
        s: LowRegister,
        n: LowRegister,
    ) -> Self::Output {
        false
    }

    fn thumb_3(&mut self, kind: Thumb3Op, d: LowRegister, n: u32) -> Self::Output {
        false
    }

    fn thumb_alu(&mut self, kind: Thumb4Op, d: LowRegister, s: LowRegister) -> Self::Output {
        false
    }

    fn thumb_hi_add(&mut self, r: (Register, Register)) -> Self::Output {
        if r.1.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn thumb_hi_cmp(&mut self, r: (Register, Register)) -> Self::Output {
        false
    }

    fn thumb_hi_mov(&mut self, r: (Register, Register)) -> Self::Output {
        if r.1.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
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
        false
    }

    fn thumb_ldrstr9(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        false
    }

    fn thumb_ldrstr10(
        &mut self,
        str: bool,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        false
    }

    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        false
    }

    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        false
    }

    fn thumb_rel_addr(&mut self, sp: bool, d: LowRegister, offset: Address) -> Self::Output {
        false
    }

    fn thumb_sp_offs(&mut self, offset: RelativeOffset) -> Self::Output {
        false
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
        false
    }

    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output {
        false
    }

    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn thumb_swi(&mut self) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn thumb_br(&mut self, offset: RelativeOffset) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn thumb_set_lr(&mut self, offset: RelativeOffset) -> Self::Output {
        false
    }

    fn thumb_bl(&mut self, offset: Address, thumb: bool) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }
}
