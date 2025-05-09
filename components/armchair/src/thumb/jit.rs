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
    optimizations::jit::InstructionTranslator,
    state::{Flag::*, LowRegister, Register},
    Cpu,
};

impl<S: Bus> ThumbVisitor for InstructionTranslator<'_, '_, '_, S> {
    type Output = ();

    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) -> Self::Output {
        todo!()
    }

    fn thumb_alu_imm(
        &mut self,
        kind: Thumb1Op,
        d: LowRegister,
        s: LowRegister,
        n: u32,
    ) -> Self::Output {
        todo!()
    }

    fn thumb_2_reg(
        &mut self,
        kind: Thumb2Op,
        d: LowRegister,
        s: LowRegister,
        n: LowRegister,
    ) -> Self::Output {
        todo!()
    }

    fn thumb_3(&mut self, kind: Thumb3Op, d: LowRegister, n: u32) -> Self::Output {
        todo!()
    }

    fn thumb_alu(&mut self, kind: Thumb4Op, d: LowRegister, s: LowRegister) -> Self::Output {
        todo!()
    }

    fn thumb_hi_add(&mut self, r: (Register, Register)) -> Self::Output {
        todo!()
    }

    fn thumb_hi_cmp(&mut self, r: (Register, Register)) -> Self::Output {
        todo!()
    }

    fn thumb_hi_mov(&mut self, r: (Register, Register)) -> Self::Output {
        todo!()
    }

    fn thumb_hi_bx(&mut self, s: Register, blx: bool) -> Self::Output {
        todo!()
    }

    fn thumb_ldr6(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        todo!()
    }

    fn thumb_ldrstr78(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        o: LowRegister,
    ) -> Self::Output {
        todo!()
    }

    fn thumb_ldrstr9(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        todo!()
    }

    fn thumb_ldrstr10(
        &mut self,
        str: bool,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        todo!()
    }

    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        todo!()
    }

    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        todo!()
    }

    fn thumb_rel_addr(&mut self, sp: bool, d: LowRegister, offset: Address) -> Self::Output {
        todo!()
    }

    fn thumb_sp_offs(&mut self, offset: RelativeOffset) -> Self::Output {
        todo!()
    }

    fn thumb_push(&mut self, reg_list: u8, lr: bool) -> Self::Output {
        todo!()
    }

    fn thumb_pop(&mut self, reg_list: u8, pc: bool) -> Self::Output {
        todo!()
    }

    fn thumb_stmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output {
        todo!()
    }

    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output {
        todo!()
    }

    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset) -> Self::Output {
        todo!()
    }

    fn thumb_swi(&mut self) -> Self::Output {
        todo!()
    }

    fn thumb_br(&mut self, offset: RelativeOffset) -> Self::Output {
        todo!()
    }

    fn thumb_set_lr(&mut self, offset: RelativeOffset) -> Self::Output {
        todo!()
    }

    fn thumb_bl(&mut self, offset: Address, thumb: bool) -> Self::Output {
        todo!()
    }
}
