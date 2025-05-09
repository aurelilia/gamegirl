pub use decode::ArmInst;
use decode::{
    ArmAluOp, ArmAluShift, ArmLdmStmConfig, ArmLdrStrConfig, ArmLdrStrOperandKind, ArmMulOp,
    ArmOperandKind, ArmQOp, ArmShMulOp, ArmSignedOperandKind,
};

use crate::{
    interface::{Bus, InstructionSet},
    memory::RelativeOffset,
    state::Register,
};

pub(crate) mod decode;
mod interpret;
mod jit;
mod trace;

pub type ArmHandler<C> = fn(&mut C, ArmInst);
pub type ArmInstructionSet<S> = InstructionSet<S, ArmInst, 4096>;

pub const fn instruction_set<S: Bus>() -> ArmInstructionSet<S> {
    InstructionSet {
        interpreter_lut: decode::get_lut_table(),
        cache_handler_lookup: |i| decode::get_instruction_handler(i, false),
    }
}

pub(crate) trait ArmVisitor {
    const IS_V5: bool;
    type Output;

    fn arm_unknown_opcode(&mut self, inst: ArmInst) -> Self::Output;
    fn arm_swi(&mut self) -> Self::Output;

    fn arm_b(&mut self, offset: RelativeOffset) -> Self::Output;
    fn arm_bl(&mut self, offset: RelativeOffset) -> Self::Output;
    fn arm_bx(&mut self, n: Register) -> Self::Output;
    fn arm_blx(&mut self, src: ArmSignedOperandKind) -> Self::Output;

    fn arm_alu_reg(
        &mut self,
        n: Register,
        d: Register,
        m: Register,
        op: ArmAluOp,
        shift_kind: ArmAluShift,
        shift_operand: ArmOperandKind,
        cpsr: bool,
    ) -> Self::Output;
    fn arm_alu_imm(
        &mut self,
        n: Register,
        d: Register,
        imm: u32,
        imm_ror: u32,
        op: ArmAluOp,
        cpsr: bool,
    ) -> Self::Output;
    fn arm_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmMulOp,
        cpsr: bool,
    ) -> Self::Output;
    fn arm_sh_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmShMulOp,
        x_top: bool,
        y_top: bool,
    ) -> Self::Output;
    fn arm_clz(&mut self, m: Register, d: Register) -> Self::Output;
    fn arm_q(&mut self, n: Register, m: Register, d: Register, op: ArmQOp) -> Self::Output;

    fn arm_msr(&mut self, src: ArmOperandKind, flags: bool, ctrl: bool, spsr: bool)
        -> Self::Output;
    fn arm_mrs(&mut self, d: Register, spsr: bool) -> Self::Output;

    fn arm_ldrstr(
        &mut self,
        n: Register,
        d: Register,
        offset: ArmLdrStrOperandKind,
        config: ArmLdrStrConfig,
    ) -> Self::Output;
    fn arm_ldmstm(
        &mut self,
        n: Register,
        rlist: u16,
        force_user: bool,
        config: ArmLdmStmConfig,
    ) -> Self::Output;
    fn arm_swp(&mut self, n: Register, d: Register, m: Register, word: bool) -> Self::Output;

    fn arm_mrc(
        &mut self,
        cm: u32,
        cp: u32,
        pn: u32,
        rd: Register,
        cn: u32,
        opc: u32,
    ) -> Self::Output;
    fn arm_mcr(
        &mut self,
        cm: u32,
        cp: u32,
        pn: u32,
        rd: Register,
        cn: u32,
        opc: u32,
    ) -> Self::Output;
}
