use super::{
    decode::*,
    ArmVisitor,
};
use crate::{
    interface::{Bus, CpuVersion},
    memory::RelativeOffset,
    optimizations::jit::InstructionTranslator,
    state::Register,
};

impl<S: Bus> ArmVisitor for InstructionTranslator<'_, '_, '_, S> {
    const IS_V5: bool = S::Version::IS_V5;

    type Output = ();

    fn arm_unknown_opcode(&mut self, inst: ArmInst) -> Self::Output {
        todo!()
    }

    fn arm_swi(&mut self) -> Self::Output {
        todo!()
    }

    fn arm_b(&mut self, offset: RelativeOffset) -> Self::Output {
        todo!()
    }

    fn arm_bl(&mut self, offset: RelativeOffset) -> Self::Output {
        todo!()
    }

    fn arm_bx(&mut self, n: Register) -> Self::Output {
        todo!()
    }

    fn arm_blx(&mut self, src: ArmSignedOperandKind) -> Self::Output {
        todo!()
    }

    fn arm_alu_reg(
        &mut self,
        n: Register,
        d: Register,
        m: Register,
        op: ArmAluOp,
        shift_kind: ArmAluShift,
        shift_operand: ArmOperandKind,
        cpsr: bool,
    ) -> Self::Output {
        todo!()
    }

    fn arm_alu_imm(
        &mut self,
        n: Register,
        d: Register,
        imm: u32,
        imm_ror: u32,
        op: ArmAluOp,
        cpsr: bool,
    ) -> Self::Output {
        todo!()
    }

    fn arm_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmMulOp,
        cpsr: bool,
    ) -> Self::Output {
        todo!()
    }

    fn arm_sh_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmShMulOp,
        x_top: bool,
        y_top: bool,
    ) -> Self::Output {
        todo!()
    }

    fn arm_clz(&mut self, m: Register, d: Register) -> Self::Output {
        todo!()
    }

    fn arm_q(&mut self, n: Register, m: Register, d: Register, op: ArmQOp) -> Self::Output {
        todo!()
    }

    fn arm_msr(
        &mut self,
        src: ArmOperandKind,
        flags: bool,
        ctrl: bool,
        spsr: bool,
    ) -> Self::Output {
        todo!()
    }

    fn arm_mrs(&mut self, d: Register, spsr: bool) -> Self::Output {
        todo!()
    }

    fn arm_ldrstr(
        &mut self,
        n: Register,
        d: Register,
        offset: ArmLdrStrOperandKind,
        config: ArmLdrStrConfig,
    ) -> Self::Output {
        todo!()
    }

    fn arm_ldmstm(
        &mut self,
        n: Register,
        rlist: u16,
        force_user: bool,
        config: ArmLdmStmConfig,
    ) -> Self::Output {
        todo!()
    }

    fn arm_swp(&mut self, n: Register, d: Register, m: Register, word: bool) -> Self::Output {
        todo!()
    }

    fn arm_mrc(
        &mut self,
        cm: u32,
        cp: u32,
        pn: u32,
        rd: Register,
        cn: u32,
        opc: u32,
    ) -> Self::Output {
        todo!()
    }

    fn arm_mcr(
        &mut self,
        cm: u32,
        cp: u32,
        pn: u32,
        rd: Register,
        cn: u32,
        opc: u32,
    ) -> Self::Output {
        todo!()
    }
}
