use alloc::{format, string::ToString};
use core::fmt::Display;

use super::{
    decode::{self, *},
    ArmVisitor,
};
use crate::{
    memory::RelativeOffset,
    misc::{self, print_op},
    registers::Register,
};

impl Display for ArmInst {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let cc = misc::condition_mnemonic(self.condition_code());
        let mut formatter = ArmFormat { f, cc };
        (decode::get_instruction_handler(*self, false))(&mut formatter, *self);
        Ok(())
    }
}

pub struct ArmFormat<'f1, 'f2> {
    f: &'f1 mut core::fmt::Formatter<'f2>,
    cc: &'static str,
}

impl<'f1, 'f2> ArmVisitor for ArmFormat<'f1, 'f2> {
    const IS_V5: bool = true;

    fn arm_unknown_opcode(&mut self, inst: ArmInst) {
        write!(self.f, "{inst:X}??").unwrap()
    }

    fn arm_swi(&mut self) {
        write!(self.f, "swi{}", self.cc).unwrap()
    }

    fn arm_b(&mut self, offset: RelativeOffset) {
        write!(self.f, "b{} {offset}", self.cc).unwrap()
    }

    fn arm_bl(&mut self, offset: RelativeOffset) {
        write!(self.f, "bl{} {offset}", self.cc).unwrap()
    }

    fn arm_bx(&mut self, n: Register) {
        write!(self.f, "bx{} {n}", self.cc).unwrap()
    }

    fn arm_blx(&mut self, src: ArmSignedOperandKind) {
        let cc = if self.cc == "nv" { "" } else { self.cc };
        match src {
            ArmSignedOperandKind::Immediate(offset) => write!(self.f, "blx{cc} {offset}").unwrap(),
            ArmSignedOperandKind::Register(reg) => write!(self.f, "blx{cc} {reg}").unwrap(),
        }
    }

    fn arm_alu_reg<const CPSR: bool>(
        &mut self,
        n: Register,
        d: Register,
        m: Register,
        op: ArmAluOp,
        shift_kind: ArmAluShift,
        shift_operand: ArmOperandKind,
    ) {
        write!(self.f, "{}{}", print_op(op), self.cc).unwrap();
        if CPSR {
            write!(self.f, "s").unwrap();
        }
        write!(self.f, " {d}, {n}, ").unwrap();
        match shift_operand {
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Lsl => write!(self.f, "{m}"),
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Lsr => {
                write!(self.f, "{m} lsr #32")
            }
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Asr => {
                write!(self.f, "{m} asr #32")
            }
            ArmOperandKind::Immediate(0) if shift_kind == ArmAluShift::Ror => {
                write!(self.f, "{m} rrx #1")
            }
            ArmOperandKind::Immediate(imm) => write!(self.f, "{m} {} #{imm}", print_op(shift_kind)),
            ArmOperandKind::Register(reg) => write!(self.f, "{m} {} {reg}", print_op(shift_kind)),
        }
        .unwrap()
    }

    fn arm_alu_imm<const CPSR: bool>(&mut self, n: Register, d: Register, imm: u32, op: ArmAluOp) {
        write!(self.f, "{}{}", print_op(op), self.cc).unwrap();
        if CPSR {
            write!(self.f, "s").unwrap();
        }
        write!(self.f, " {d}, {n}, #{imm}").unwrap();
    }

    fn arm_mul<const OP: ArmMulOp>(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        cpsr: bool,
    ) {
        write!(self.f, "{}{}", print_op(OP), self.cc).unwrap();
        if cpsr {
            write!(self.f, "s").unwrap();
        }
        if OP == ArmMulOp::Mul || OP == ArmMulOp::Mla {
            write!(self.f, " {d}, {m}, {s}").unwrap();
        } else {
            write!(self.f, " {n}, {d}, {m}, {s}").unwrap();
        }
    }

    fn arm_sh_mul<const OP: ArmShMulOp>(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        x_top: bool,
        _y_top: bool,
    ) {
        match OP {
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
        .unwrap()
    }

    fn arm_clz(&mut self, m: Register, d: Register) {
        write!(self.f, "clz{} {d}, {m}", self.cc).unwrap()
    }

    fn arm_q<const OP: ArmQOp>(&mut self, n: Register, m: Register, d: Register) {
        write!(self.f, "{}{} {d}, {n} {m}", print_op(OP), self.cc).unwrap()
    }

    fn arm_msr(&mut self, src: ArmOperandKind, flags: bool, ctrl: bool, spsr: bool) {
        write!(
            self.f,
            "msr{} {}",
            self.cc,
            if spsr { "spsr" } else { "cpsr" }
        )
        .unwrap();
        if ctrl {
            write!(self.f, "_ctrl").unwrap();
        }
        if flags {
            write!(self.f, "_flg").unwrap();
        }

        match src {
            ArmOperandKind::Immediate(imm) => write!(self.f, ", #0x{imm:X}"),
            ArmOperandKind::Register(reg) => write!(self.f, ", {reg}"),
        }
        .unwrap()
    }

    fn arm_mrs(&mut self, d: Register, spsr: bool) {
        write!(
            self.f,
            "mrs{} {d}, {}",
            self.cc,
            if spsr { "spsr" } else { "cpsr" }
        )
        .unwrap()
    }

    fn arm_ldrstr(
        &mut self,
        n: Register,
        d: Register,
        offset: ArmLdrStrOperandKind,
        config: ArmLdrStrConfig,
    ) {
        match config.kind {
            ArmLdrStrKind::LoadByte => write!(self.f, "ldr{}b", self.cc).unwrap(),
            ArmLdrStrKind::LoadSignedByte => write!(self.f, "ldr{}sb", self.cc).unwrap(),
            ArmLdrStrKind::LoadHalfword => write!(self.f, "ldr{}h", self.cc).unwrap(),
            ArmLdrStrKind::LoadSignedHalfword => write!(self.f, "ldr{}sh", self.cc).unwrap(),
            ArmLdrStrKind::LoadWord => write!(self.f, "ldr{}", self.cc).unwrap(),
            ArmLdrStrKind::LoadDoubleWord => write!(self.f, "ldr{}d", self.cc).unwrap(),
            ArmLdrStrKind::StoreByte => write!(self.f, "str{}b", self.cc).unwrap(),
            ArmLdrStrKind::StoreHalfword => write!(self.f, "str{}h", self.cc).unwrap(),
            ArmLdrStrKind::StoreWord => write!(self.f, "str{}", self.cc).unwrap(),
            ArmLdrStrKind::StoreDoubleWord => write!(self.f, "str{}d", self.cc).unwrap(),
        }
        write!(self.f, " {d}").unwrap();

        let shift = 'shift: {
            let base = match offset {
                ArmLdrStrOperandKind::Immediate(0) => break 'shift "".to_string(),
                ArmLdrStrOperandKind::Immediate(imm) => format!("#0x{imm:X}"),
                ArmLdrStrOperandKind::Register(reg) => format!("{reg}"),
                ArmLdrStrOperandKind::ShiftedRegister { base, shift, by } => {
                    format!("{base} {} #{by}", print_op(shift))
                }
            };
            let op = if config.up { "+" } else { "-" };
            format!(" {op}{base}")
        };

        if config.pre {
            write!(self.f, ", [{n}{shift}]").unwrap();
            if config.writeback {
                write!(self.f, "!").unwrap();
            }
        } else {
            write!(self.f, ", [{n}], {shift}").unwrap()
        }
    }

    fn arm_ldmstm(&mut self, n: Register, rlist: u16, force_user: bool, config: ArmLdmStmConfig) {
        write!(
            self.f,
            "{}{}",
            if config.ldr { "ldr" } else { "str" },
            self.cc
        )
        .unwrap();
        let kind = match (config.pre, config.up) {
            (true, true) => "ib",
            (true, false) => "db",
            (false, true) => "ia",
            (false, false) => "da",
        };
        write!(self.f, "{} {n}", kind).unwrap();
        if config.writeback {
            write!(self.f, "!").unwrap();
        }
        write!(self.f, ",").unwrap();
        for r in Register::from_rlist(rlist) {
            write!(self.f, " {r}").unwrap()
        }
        if force_user {
            write!(self.f, " ^").unwrap()
        }
    }

    fn arm_swp<const WORD: bool>(&mut self, n: Register, d: Register, m: Register) {
        if WORD {
            write!(self.f, "swp{} {d}, {m}, [{n}]", self.cc).unwrap()
        } else {
            write!(self.f, "swp{}b {d}, {m}, [{n}]", self.cc).unwrap()
        }
    }

    fn arm_mrc(&mut self, cm: u32, cp: u32, pn: u32, rd: Register, cn: u32, opc: u32) {
        write!(
            self.f,
            "mrc{} P{pn}, {opc}, {rd}, C{cn}, C{cm}, {cp}",
            self.cc
        )
        .unwrap()
    }

    fn arm_mcr(&mut self, cm: u32, cp: u32, pn: u32, rd: Register, cn: u32, opc: u32) {
        write!(
            self.f,
            "mcr{} P{pn}, {opc}, {rd}, C{cn}, C{cm}, {cp}",
            self.cc
        )
        .unwrap()
    }
}
