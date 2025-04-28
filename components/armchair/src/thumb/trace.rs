use core::fmt::Display;

use super::{
    decode::{self, *},
    ThumbVisitor,
};
use crate::{
    memory::{Address, RelativeOffset},
    misc::{self, print_op},
    state::{LowRegister, Register},
};

impl Display for ThumbInst {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut formatter = ThumbFormat { f };
        (decode::get_instruction_handler(*self))(&mut formatter, *self);
        Ok(())
    }
}

pub struct ThumbFormat<'f1, 'f2> {
    f: &'f1 mut core::fmt::Formatter<'f2>,
}

impl<'f1, 'f2> ThumbVisitor for ThumbFormat<'f1, 'f2> {
    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) {
        write!(self.f, "{inst:X}??").unwrap()
    }

    fn thumb_alu_imm(&mut self, kind: Thumb1Op, d: LowRegister, s: LowRegister, n: u32) {
        write!(self.f, "{} {d}, {s}, ${n}", print_op(kind)).unwrap()
    }

    fn thumb_2_reg(&mut self, kind: Thumb2Op, d: LowRegister, s: LowRegister, n: LowRegister) {
        write!(self.f, "{} {d}, {s}, {n}", print_op(kind)).unwrap()
    }

    fn thumb_3(&mut self, kind: Thumb3Op, d: LowRegister, n: u32) {
        write!(self.f, "{} {d}, $0x{n:X}", print_op(kind)).unwrap()
    }

    fn thumb_alu(&mut self, kind: Thumb4Op, d: LowRegister, s: LowRegister) {
        match kind {
            Thumb4Op::Tst => write!(self.f, "tst {s}"),
            _ => write!(self.f, "{} {d}, {s}", print_op(kind)),
        }
        .unwrap()
    }

    fn thumb_hi_add(&mut self, (s, d): (Register, Register)) {
        write!(self.f, "add {d}, {s}").unwrap()
    }

    fn thumb_hi_cmp(&mut self, (s, d): (Register, Register)) {
        write!(self.f, "cmp {d}, {s}").unwrap()
    }

    fn thumb_hi_mov(&mut self, (s, d): (Register, Register)) {
        write!(self.f, "mov {d}, {s}").unwrap()
    }

    fn thumb_hi_bx(&mut self, s: Register, blx: bool) {
        if blx {
            write!(self.f, "blx {s}").unwrap()
        } else {
            write!(self.f, "bx {s}").unwrap()
        }
    }

    fn thumb_ldr6(&mut self, d: LowRegister, offset: Address) {
        write!(self.f, "ldr {d}, [PC, {offset}]").unwrap()
    }

    fn thumb_ldrstr78(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        o: LowRegister,
    ) {
        write!(self.f, "{} {d}, [{b}, {o}]", print_op(op)).unwrap()
    }

    fn thumb_ldrstr9(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) {
        write!(self.f, "{} {d}, [{b}, {offset}]", print_op(op)).unwrap()
    }

    fn thumb_ldrstr10(&mut self, str: bool, d: LowRegister, b: LowRegister, offset: Address) {
        if str {
            write!(self.f, "strh {d}, [{b}, {offset}]")
        } else {
            write!(self.f, "ldrh {d}, [{b}, {offset}]")
        }
        .unwrap()
    }

    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address) {
        write!(self.f, "str {d}, [sp, {offset}]").unwrap()
    }

    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address) {
        write!(self.f, "ldr {d}, [sp, {offset}]").unwrap()
    }

    fn thumb_rel_addr(&mut self, sp: bool, d: LowRegister, offset: Address) {
        if sp {
            write!(self.f, "add {d}, sp, {offset}")
        } else {
            write!(self.f, "add {d}, pc, {offset}")
        }
        .unwrap()
    }

    fn thumb_sp_offs(&mut self, offset: RelativeOffset) {
        write!(self.f, "add sp, {offset}").unwrap()
    }

    fn thumb_push(&mut self, reg_list: u8, lr: bool) {
        write!(self.f, "push").unwrap();
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}").unwrap()
        }
        if lr {
            write!(self.f, " lr").unwrap()
        }
    }

    fn thumb_pop(&mut self, reg_list: u8, pc: bool) {
        write!(self.f, "pop").unwrap();
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}").unwrap()
        }
        if pc {
            write!(self.f, " pc").unwrap()
        }
    }

    fn thumb_stmia(&mut self, b: LowRegister, reg_list: u8) {
        write!(self.f, "stmia {b}!,").unwrap();
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}").unwrap()
        }
    }

    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8) {
        write!(self.f, "ldmia {b}!,").unwrap();
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}").unwrap()
        }
    }

    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset) {
        write!(
            self.f,
            "b{} {offset}",
            misc::condition_mnemonic(cond).to_ascii_lowercase()
        )
        .unwrap()
    }

    fn thumb_swi(&mut self) {
        write!(self.f, "swi").unwrap()
    }

    fn thumb_br(&mut self, offset: RelativeOffset) {
        write!(self.f, "b {offset}",).unwrap()
    }

    fn thumb_set_lr(&mut self, offset: RelativeOffset) {
        write!(self.f, "mov lr, (pc + {offset})",).unwrap()
    }

    fn thumb_bl(&mut self, offset: Address, thumb: bool) {
        if thumb {
            write!(self.f, "bl lr + {offset}")
        } else {
            write!(self.f, "blx lr + {offset}")
        }
        .unwrap()
    }
}
