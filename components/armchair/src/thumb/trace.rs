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
        (decode::get_instruction_handler(*self))(&mut formatter, *self)
    }
}

pub struct ThumbFormat<'f1, 'f2> {
    f: &'f1 mut core::fmt::Formatter<'f2>,
}

impl<'f1, 'f2> ThumbVisitor for ThumbFormat<'f1, 'f2> {
    type Output = core::fmt::Result;

    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) -> Self::Output {
        write!(self.f, "{inst:X}??")
    }

    fn thumb_alu_imm(
        &mut self,
        kind: Thumb1Op,
        d: LowRegister,
        s: LowRegister,
        n: u32,
    ) -> Self::Output {
        write!(self.f, "{} {d}, {s}, ${n}", print_op(kind))
    }

    fn thumb_2_reg(
        &mut self,
        kind: Thumb2Op,
        d: LowRegister,
        s: LowRegister,
        n: LowRegister,
    ) -> Self::Output {
        write!(self.f, "{} {d}, {s}, {n}", print_op(kind))
    }

    fn thumb_3(&mut self, kind: Thumb3Op, d: LowRegister, n: u32) -> Self::Output {
        write!(self.f, "{} {d}, $0x{n:X}", print_op(kind))
    }

    fn thumb_alu(&mut self, kind: Thumb4Op, d: LowRegister, s: LowRegister) -> Self::Output {
        match kind {
            Thumb4Op::Tst => write!(self.f, "tst {s}"),
            _ => write!(self.f, "{} {d}, {s}", print_op(kind)),
        }
    }

    fn thumb_hi_add(&mut self, (s, d): (Register, Register)) -> Self::Output {
        write!(self.f, "add {d}, {s}")
    }

    fn thumb_hi_cmp(&mut self, (s, d): (Register, Register)) -> Self::Output {
        write!(self.f, "cmp {d}, {s}")
    }

    fn thumb_hi_mov(&mut self, (s, d): (Register, Register)) -> Self::Output {
        write!(self.f, "mov {d}, {s}")
    }

    fn thumb_hi_bx(&mut self, s: Register, blx: bool) -> Self::Output {
        if blx {
            write!(self.f, "blx {s}")
        } else {
            write!(self.f, "bx {s}")
        }
    }

    fn thumb_ldr6(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        write!(self.f, "ldr {d}, [PC, {offset}]")
    }

    fn thumb_ldrstr78(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        o: LowRegister,
    ) -> Self::Output {
        write!(self.f, "{} {d}, [{b}, {o}]", print_op(op))
    }

    fn thumb_ldrstr9(
        &mut self,
        op: ThumbStrLdrOp,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        write!(self.f, "{} {d}, [{b}, {offset}]", print_op(op))
    }

    fn thumb_ldrstr10(
        &mut self,
        str: bool,
        d: LowRegister,
        b: LowRegister,
        offset: Address,
    ) -> Self::Output {
        if str {
            write!(self.f, "strh {d}, [{b}, {offset}]")
        } else {
            write!(self.f, "ldrh {d}, [{b}, {offset}]")
        }
    }

    fn thumb_str_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        write!(self.f, "str {d}, [sp, {offset}]")
    }

    fn thumb_ldr_sp(&mut self, d: LowRegister, offset: Address) -> Self::Output {
        write!(self.f, "ldr {d}, [sp, {offset}]")
    }

    fn thumb_rel_addr(&mut self, sp: bool, d: LowRegister, offset: Address) -> Self::Output {
        if sp {
            write!(self.f, "add {d}, sp, {offset}")
        } else {
            write!(self.f, "add {d}, pc, {offset}")
        }
    }

    fn thumb_sp_offs(&mut self, offset: RelativeOffset) -> Self::Output {
        write!(self.f, "add sp, {offset}")
    }

    fn thumb_push(&mut self, reg_list: u8, lr: bool) -> Self::Output {
        write!(self.f, "push")?;
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}")?
        }
        if lr {
            write!(self.f, " lr")?;
        }
        Ok(())
    }

    fn thumb_pop(&mut self, reg_list: u8, pc: bool) -> Self::Output {
        write!(self.f, "pop")?;
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}")?
        }
        if pc {
            write!(self.f, " pc")?;
        }
        Ok(())
    }

    fn thumb_stmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output {
        write!(self.f, "stmia {b}!,")?;
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}")?
        }
        Ok(())
    }

    fn thumb_ldmia(&mut self, b: LowRegister, reg_list: u8) -> Self::Output {
        write!(self.f, "ldmia {b}!,")?;
        for r in LowRegister::from_rlist(reg_list) {
            write!(self.f, " {r}")?
        }
        Ok(())
    }

    fn thumb_bcond(&mut self, cond: u16, offset: RelativeOffset) -> Self::Output {
        write!(
            self.f,
            "b{} {offset}",
            misc::condition_mnemonic(cond).to_ascii_lowercase()
        )
    }

    fn thumb_swi(&mut self) -> Self::Output {
        write!(self.f, "swi")
    }

    fn thumb_br(&mut self, offset: RelativeOffset) -> Self::Output {
        write!(self.f, "b {offset}",)
    }

    fn thumb_set_lr(&mut self, offset: RelativeOffset) -> Self::Output {
        write!(self.f, "mov lr, (pc + {offset})",)
    }

    fn thumb_bl(&mut self, offset: Address, thumb: bool) -> Self::Output {
        if thumb {
            write!(self.f, "bl lr + {offset}")
        } else {
            write!(self.f, "blx lr + {offset}")
        }
    }
}
