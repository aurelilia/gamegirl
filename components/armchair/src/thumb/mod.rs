pub use decode::ThumbInst;
use decode::{Thumb1Op, Thumb2Op, Thumb3Op, Thumb4Op, ThumbStrLdrOp};

use crate::{
    interface::{Bus, InstructionSet},
    memory::{Address, RelativeOffset},
    state::{LowRegister, Register},
};

pub(crate) mod decode;
mod interpret;
mod jit;
mod trace;

pub type ThumbHandler<C> = fn(&mut C, ThumbInst);
pub type ThumbInstructionSet<S> = InstructionSet<S, ThumbInst, 256>;

pub const fn instruction_set<S: Bus>() -> ThumbInstructionSet<S> {
    InstructionSet {
        interpreter_lut: decode::get_lut_table(),
        cache_handler_lookup: |i| decode::get_instruction_handler(i),
    }
}

pub(crate) trait ThumbVisitor {
    type Output;

    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) -> Self::Output;
    fn thumb_alu_imm(
        &mut self,
        kind: Thumb1Op,
        d: LowRegister,
        s: LowRegister,
        n: u32,
    ) -> Self::Output;
    fn thumb_2_reg(
        &mut self,
        kind: Thumb2Op,
        d: LowRegister,
        s: LowRegister,
        n: LowRegister,
    ) -> Self::Output;
    fn thumb_3(&mut self, kind: Thumb3Op, d: LowRegister, n: u32) -> Self::Output;
    fn thumb_alu(&mut self, kind: Thumb4Op, d: LowRegister, s: LowRegister) -> Self::Output;
    fn thumb_hi_add(&mut self, r: (Register, Register)) -> Self::Output;
    fn thumb_hi_cmp(&mut self, r: (Register, Register)) -> Self::Output;
    fn thumb_hi_mov(&mut self, r: (Register, Register)) -> Self::Output;
    fn thumb_hi_bx(&mut self, s: Register, blx: bool) -> Self::Output;
    fn thumb_ldr6(&mut self, d: LowRegister, offset: Address) -> Self::Output;
    fn thumb_ldrstr78(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        o: LowRegister,
    ) -> Self::Output;
    fn thumb_ldrstr9(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output;
    fn thumb_ldrstr10(
        &mut self,
        str: bool,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output;
    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output;
    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output;
    fn thumb_rel_addr(&mut self, sp: bool, d: LowRegister, offset: Address) -> Self::Output;
    fn thumb_sp_offs(&mut self, offset: RelativeOffset) -> Self::Output;
    fn thumb_push(&mut self, reg_list: u8, lr: bool) -> Self::Output;
    fn thumb_pop(&mut self, reg_list: u8, pc: bool) -> Self::Output;
    fn thumb_stmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output;
    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output;
    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset) -> Self::Output;
    fn thumb_swi(&mut self) -> Self::Output;
    fn thumb_br(&mut self, offset: RelativeOffset) -> Self::Output;
    fn thumb_set_lr(&mut self, offset: RelativeOffset) -> Self::Output;
    fn thumb_bl(&mut self, offset: Address, thumb: bool) -> Self::Output;
}
