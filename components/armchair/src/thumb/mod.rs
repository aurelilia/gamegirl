pub use decode::ThumbInst;
use decode::{Thumb1Op, Thumb2Op, Thumb3Op, Thumb4Op, ThumbStrLdrOp};

use crate::{
    interface::{Bus, InstructionSet},
    memory::{Address, RelativeOffset},
    state::{LowRegister, Register},
};

mod decode;
mod execute;
mod trace;

pub type ThumbHandler<C> = fn(&mut C, ThumbInst);
pub type ThumbInstructionSet<S> = InstructionSet<S, ThumbInst, 256>;

pub const fn instruction_set<S: Bus>() -> ThumbInstructionSet<S> {
    InstructionSet {
        interpreter_lut: decode::get_lut_table(),
        cache_handler_lookup: |i| decode::get_instruction_handler(i, false),
    }
}

trait ThumbVisitor {
    fn thumb_unknown_opcode(&mut self, inst: ThumbInst);
    fn thumb_alu_imm<const KIND: Thumb1Op>(&mut self, d: LowRegister, s: LowRegister, n: u32);
    fn thumb_2_reg<const KIND: Thumb2Op>(&mut self, d: LowRegister, s: LowRegister, n: LowRegister);
    fn thumb_3<const KIND: Thumb3Op>(&mut self, d: LowRegister, n: u32);
    fn thumb_alu<const KIND: Thumb4Op>(&mut self, d: LowRegister, s: LowRegister);
    // TODO maybe fold these?
    fn thumb_hi_add(&mut self, r: (Register, Register));
    fn thumb_hi_cmp(&mut self, r: (Register, Register));
    fn thumb_hi_mov(&mut self, r: (Register, Register));
    fn thumb_hi_bx(&mut self, s: Register, blx: bool);
    // TODO fold these
    fn thumb_ldr6(&mut self, d: LowRegister, offset: Address);
    fn thumb_ldrstr78<const O: ThumbStrLdrOp>(
        &mut self,
        d: LowRegister,
        b: LowRegister,
        o: LowRegister,
    );
    fn thumb_ldrstr9<const O: ThumbStrLdrOp>(
        &mut self,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    );
    fn thumb_ldrstr10<const STR: bool>(&mut self, d: LowRegister, b: LowRegister, offset: Address);
    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address);
    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address);
    fn thumb_rel_addr<const SP: bool>(&mut self, d: LowRegister, offset: Address);
    fn thumb_sp_offs(&mut self, offset: RelativeOffset);
    fn thumb_push(&mut self, reg_list: u8, lr: bool);
    fn thumb_pop(&mut self, reg_list: u8, pc: bool);
    fn thumb_stmia(&mut self, b: LowRegister, reg_list: u8);
    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8);
    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset);
    fn thumb_swi(&mut self);
    fn thumb_br(&mut self, offset: RelativeOffset);
    fn thumb_set_lr(&mut self, offset: RelativeOffset);
    fn thumb_bl<const THUMB: bool>(&mut self, offset: Address);
}
