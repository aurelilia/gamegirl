pub use decode::ArmInst;
use decode::{
    ArmAluOp, ArmAluShift, ArmLdmStmConfig, ArmLdrStrConfig, ArmLdrStrOperandKind, ArmMulOp,
    ArmOperandKind, ArmQOp, ArmShMulOp, ArmSignedOperandKind,
};

use crate::{
    interface::{Bus, InstructionSet},
    memory::RelativeOffset,
    registers::Register,
};

mod decode;
mod execute;
mod trace;

pub type ArmHandler<C> = fn(&mut C, ArmInst);
pub type ArmInstructionSet<S> = InstructionSet<S, ArmInst, 4096>;

pub const fn instruction_set<S: Bus>() -> ArmInstructionSet<S> {
    InstructionSet {
        interpreter_lut: decode::get_lut_table(),
        cache_handler_lookup: |i| decode::get_instruction_handler(i, false),
    }
}

trait ArmVisitor {
    const IS_V5: bool;

    fn arm_unknown_opcode(&mut self, inst: ArmInst);
    fn arm_swi(&mut self);

    fn arm_b(&mut self, offset: RelativeOffset);
    fn arm_bl(&mut self, offset: RelativeOffset);
    fn arm_bx(&mut self, n: Register);
    fn arm_blx(&mut self, src: ArmSignedOperandKind);

    fn arm_alu_reg<const CPSR: bool>(
        &mut self,
        n: Register,
        d: Register,
        m: Register,
        op: ArmAluOp,
        shift_kind: ArmAluShift,
        shift_operand: ArmOperandKind,
    );
    fn arm_alu_imm<const CPSR: bool>(&mut self, n: Register, d: Register, imm: u32, op: ArmAluOp);
    fn arm_mul<const OP: ArmMulOp>(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        cpsr: bool,
    );
    fn arm_sh_mul<const OP: ArmShMulOp>(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        x_top: bool,
        y_top: bool,
    );

    fn arm_clz(&mut self, m: Register, d: Register);
    fn arm_q<const OP: ArmQOp>(&mut self, n: Register, m: Register, d: Register);

    fn arm_msr(&mut self, src: ArmOperandKind, flags: bool, ctrl: bool, spsr: bool);
    fn arm_mrs(&mut self, d: Register, spsr: bool);

    fn arm_ldrstr(
        &mut self,
        n: Register,
        d: Register,
        offset: ArmLdrStrOperandKind,
        config: ArmLdrStrConfig,
    );
    fn arm_ldmstm(&mut self, n: Register, rlist: u16, force_user: bool, config: ArmLdmStmConfig);
    fn arm_swp<const WORD: bool>(&mut self, n: Register, d: Register, m: Register);

    fn arm_mrc(&mut self, cm: u32, cp: u32, pn: u32, rd: Register, cn: u32, opc: u32);
    fn arm_mcr(&mut self, cm: u32, cp: u32, pn: u32, rd: Register, cn: u32, opc: u32);
}
