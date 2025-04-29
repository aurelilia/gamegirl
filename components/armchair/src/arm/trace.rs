use alloc::{format, string::ToString};
use core::fmt::Display;

use super::{
    decode::{self, *},
    ArmVisitor,
};
use crate::{
    memory::RelativeOffset,
    misc::{self, print_op},
    state::Register,
};

impl Display for ArmInst {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let cc = misc::condition_mnemonic(self.condition_code());
        let mut formatter = ArmFormat { f, cc };
        (decode::get_instruction_handler(*self, false))(&mut formatter, *self)
    }
}

pub struct ArmFormat<'f1, 'f2> {
    f: &'f1 mut core::fmt::Formatter<'f2>,
    cc: &'static str,
}

impl<'f1, 'f2> ArmVisitor for ArmFormat<'f1, 'f2> {
    const IS_V5: bool = true;
    type Output = core::fmt::Result;

    fn arm_unknown_opcode(&mut self, inst: ArmInst) -> Self::Output {
        write!(self.f, "{inst:X}??")
    }

    fn arm_swi(&mut self) -> Self::Output {
        write!(self.f, "swi{}", self.cc)
    }

    fn arm_b(&mut self, offset: RelativeOffset) -> Self::Output {
        write!(self.f, "b{} {offset}", self.cc)
    }

    fn arm_bl(&mut self, offset: RelativeOffset) -> Self::Output {
        write!(self.f, "bl{} {offset}", self.cc)
    }

    fn arm_bx(&mut self, n: Register) -> Self::Output {
        write!(self.f, "bx{} {n}", self.cc)
    }

    fn arm_blx(&mut self, src: ArmSignedOperandKind) -> Self::Output {
        let cc = if self.cc == "nv" { "" } else { self.cc };
        match src {
            ArmSignedOperandKind::Immediate(offset) => write!(self.f, "blx{cc} {offset}"),
            ArmSignedOperandKind::Register(reg) => write!(self.f, "blx{cc} {reg}"),
        }
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
        write!(self.f, "{}{}", print_op(op), self.cc)?;
        if cpsr {
            write!(self.f, "s")?;
        }
        write!(self.f, " {d}, {n}, ")?;
        match shift_operand {
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Lsl => write!(self.f, "{m}"),
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Lsr => {
                write!(self.f, "{m}, lsr $32")
            }
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Asr => {
                write!(self.f, "{m}, asr $32")
            }
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Ror => {
                write!(self.f, "{m}, rrx $1")
            }
            ArmOperandKind::Immediate(imm) => {
                write!(self.f, "{m}, {} ${imm}", print_op(shift_kind))
            }
            ArmOperandKind::Register(reg) => write!(self.f, "{m}, {} {reg}", print_op(shift_kind)),
        }
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
        write!(self.f, "{}{}", print_op(op), self.cc)?;
        if cpsr {
            write!(self.f, "s")?;
        }
        let imm = imm.rotate_right(imm_ror);
        write!(self.f, " {d}, {n}, ${imm}")
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
        write!(self.f, "{}{}", print_op(op), self.cc)?;
        if cpsr {
            write!(self.f, "s")?;
        }
        match op {
            ArmMulOp::Mul => write!(self.f, " {d}, {m}, {s}"),
            ArmMulOp::Mla => write!(self.f, " {d}, {m}, {s}, {n}"),
            _ => write!(self.f, " {n}, {d}, {m}, {s}"),
        }
    }

    fn arm_sh_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmShMulOp,
        x_top: bool,
        _y_top: bool,
    ) -> Self::Output {
        match op {
            ArmShMulOp::SmlaXy => write!(self.f, "smlaxy{} {d}, {m}, {s}, {n}", self.cc),
            ArmShMulOp::SmlawYOrSmulwY if x_top => {
                write!(self.f, "smulw{} {d}, {m}, {s}", self.cc)
            }
            ArmShMulOp::SmlawYOrSmulwY => {
                write!(self.f, "smlaw{} {d}, {m}, {s}, {n}", self.cc)
            }
            ArmShMulOp::SmlalXy => write!(self.f, "smlalxy{} {n}, {d}, {m}, {s}", self.cc),
            ArmShMulOp::SmulXy => write!(self.f, "smulxy{} {d}, {m}, {s}", self.cc),
        }
    }

    fn arm_clz(&mut self, m: Register, d: Register) -> Self::Output {
        write!(self.f, "clz{} {d}, {m}", self.cc)
    }

    fn arm_q(&mut self, n: Register, m: Register, d: Register, op: ArmQOp) -> Self::Output {
        write!(self.f, "{}{} {d}, {n} {m}", print_op(op), self.cc)
    }

    fn arm_msr(
        &mut self,
        src: ArmOperandKind,
        flags: bool,
        ctrl: bool,
        spsr: bool,
    ) -> Self::Output {
        write!(
            self.f,
            "msr{} {}",
            self.cc,
            if spsr { "spsr" } else { "cpsr" }
        )?;
        if ctrl {
            write!(self.f, "_ctrl")?;
        }
        if flags {
            write!(self.f, "_flg")?;
        }

        match src {
            ArmOperandKind::Immediate(imm) => write!(self.f, ", $0x{imm:X}"),
            ArmOperandKind::Register(reg) => write!(self.f, ", {reg}"),
        }
    }

    fn arm_mrs(&mut self, d: Register, spsr: bool) -> Self::Output {
        write!(
            self.f,
            "mrs{} {d}, {}",
            self.cc,
            if spsr { "spsr" } else { "cpsr" }
        )
    }

    fn arm_ldrstr(
        &mut self,
        n: Register,
        d: Register,
        offset: ArmLdrStrOperandKind,
        config: ArmLdrStrConfig,
    ) -> Self::Output {
        match config.kind {
            ArmLdrStrKind::LoadByte => write!(self.f, "ldr{}b", self.cc),
            ArmLdrStrKind::LoadSignedByte => write!(self.f, "ldr{}sb", self.cc),
            ArmLdrStrKind::LoadHalfword => write!(self.f, "ldr{}h", self.cc),
            ArmLdrStrKind::LoadSignedHalfword => write!(self.f, "ldr{}sh", self.cc),
            ArmLdrStrKind::LoadWord => write!(self.f, "ldr{}", self.cc),
            ArmLdrStrKind::LoadDoubleWord => write!(self.f, "ldr{}d", self.cc),
            ArmLdrStrKind::StoreByte => write!(self.f, "str{}b", self.cc),
            ArmLdrStrKind::StoreHalfword => write!(self.f, "str{}h", self.cc),
            ArmLdrStrKind::StoreWord => write!(self.f, "str{}", self.cc),
            ArmLdrStrKind::StoreDoubleWord => write!(self.f, "str{}d", self.cc),
        }?;
        write!(self.f, " {d}")?;

        let shift = 'shift: {
            let base = match offset {
                ArmLdrStrOperandKind::Immediate(0) => break 'shift "".to_string(),
                ArmLdrStrOperandKind::Immediate(imm) => format!("$0x{imm:X}"),
                ArmLdrStrOperandKind::Register(reg) => format!("{reg}"),
                ArmLdrStrOperandKind::ShiftedRegister { base, shift, by } => {
                    format!("{base}, {} ${by}", print_op(shift))
                }
            };
            let op = if config.up { "" } else { "-" };
            format!(", {op}{base}")
        };

        if config.pre {
            write!(self.f, ", [{n}{shift}]")?;
            if config.writeback {
                write!(self.f, "!")?;
            }
        } else {
            write!(self.f, ", [{n}]{shift}")?;
        }
        Ok(())
    }

    fn arm_ldmstm(
        &mut self,
        n: Register,
        rlist: u16,
        force_user: bool,
        config: ArmLdmStmConfig,
    ) -> Self::Output {
        write!(
            self.f,
            "{}{}",
            if config.ldr { "ldm" } else { "stm" },
            self.cc
        )?;
        let kind = match (config.pre, config.up) {
            (true, true) => "ib",
            (true, false) => "db",
            (false, true) => "ia",
            (false, false) => "da",
        };
        write!(self.f, "{} {n}", kind)?;
        if config.writeback {
            write!(self.f, "!")?;
        }
        let mut regs = Register::from_rlist(rlist);
        let first = regs.next().unwrap_or(Register(0));
        write!(self.f, ", {{{first}")?;
        for r in regs {
            write!(self.f, ", {r}")?;
        }
        write!(self.f, "}}")?;
        if force_user {
            write!(self.f, " ^")?;
        }
        Ok(())
    }

    fn arm_swp(&mut self, n: Register, d: Register, m: Register, word: bool) -> Self::Output {
        if word {
            write!(self.f, "swp{} {d}, {m}, [{n}]", self.cc)
        } else {
            write!(self.f, "swp{}b {d}, {m}, [{n}]", self.cc)
        }
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
        write!(
            self.f,
            "mrc{} P{pn}, {opc}, {rd}, C{cn}, C{cm}, {cp}",
            self.cc
        )
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
        write!(
            self.f,
            "mcr{} P{pn}, {opc}, {rd}, C{cn}, C{cm}, {cp}",
            self.cc
        )
    }
}
