use core::{fmt::UpperHex, marker::ConstParamTy, ops::RangeInclusive};

use bitmatch::bitmatch;
use common::numutil::{NumExt, U32Ext};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::ArmVisitor;
use crate::{memory::RelativeOffset, registers::Register};

#[derive(Copy, Clone)]
pub struct ArmInst(u32);

impl ArmInst {
    pub fn of(inst: u32) -> Self {
        Self(inst)
    }

    pub fn reg(self, idx: u32) -> Register {
        Register(self.0.bits(idx, 4) as u16)
    }

    pub fn is_bit(self, bit: u16) -> bool {
        self.0.is_bit(bit)
    }

    pub fn condition_code(self) -> u16 {
        (self.0 >> 28) as u16
    }

    #[inline]
    const fn bits(self, range: RangeInclusive<u32>) -> u32 {
        (self.0 >> *range.start()) & ((1 << ((*range.end() - *range.start()) + 1)) - 1)
    }

    fn safe_transmute<T: FromPrimitive>(self, range: RangeInclusive<u32>) -> T {
        T::from_u32(self.bits(range)).unwrap()
    }

    fn ldrstr_config(self, kind: ArmLdrStrKind) -> ArmLdrStrConfig {
        ArmLdrStrConfig {
            pre: self.is_bit(24),
            up: self.is_bit(23),
            kind,
            writeback: self.is_bit(21),
        }
    }
    fn ldrstr_shift_reg(self) -> ArmLdrStrOperandKind {
        ArmLdrStrOperandKind::ShiftedRegister {
            base: self.reg(0),
            shift: self.safe_transmute(5..=6),
            by: self.bits(7..=11),
        }
    }
    fn ldrstr_split_imm(self) -> ArmLdrStrOperandKind {
        ArmLdrStrOperandKind::Immediate(self.0 & 0xF | ((self.0 >> 4) & 0xF0))
    }
}

impl UpperHex for ArmInst {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        UpperHex::fmt(&self.0, f)
    }
}

#[derive(FromPrimitive, ConstParamTy, Debug, PartialEq, Eq, PartialOrd)]
pub enum ArmAluOp {
    And,
    Eor,
    Sub,
    Rsb,
    Add,
    Adc,
    Sbc,
    Rsc,
    Tst,
    Teq,
    Cmp,
    Cmn,
    Orr,
    Mov,
    Bic,
    Mvn,
}

impl ArmAluOp {
    pub fn should_write(self) -> bool {
        self < Self::Tst || self > Self::Cmn
    }
}

#[derive(FromPrimitive, ConstParamTy, Debug, PartialEq, Eq)]
pub enum ArmAluShift {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

pub enum ArmOperandKind {
    Immediate(u32),
    Register(Register),
}

pub enum ArmSignedOperandKind {
    Immediate(RelativeOffset),
    Register(Register),
}

pub enum ArmLdrStrOperandKind {
    Immediate(u32),
    Register(Register),
    ShiftedRegister {
        base: Register,
        shift: ArmAluShift,
        by: u32,
    },
}

#[derive(ConstParamTy, Debug, PartialEq, Eq)]
pub enum ArmMulOp {
    Mul,
    Mla,
    Umaal,
    Umull,
    Umlal,
    Smull,
    Smlal,
}

#[derive(ConstParamTy, Debug, PartialEq, Eq)]
pub enum ArmShMulOp {
    SmlaXy,
    SmlawYOrSmulwY,
    SmlalXy,
    SmulXy,
}

#[derive(ConstParamTy, Debug, PartialEq, Eq)]
pub enum ArmQOp {
    Qadd,
    Qsub,
    QdAdd,
    QdSub,
}

pub struct ArmLdrStrConfig {
    pub pre: bool,
    pub up: bool,
    pub kind: ArmLdrStrKind,
    pub writeback: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum ArmLdrStrKind {
    LoadByte,
    LoadSignedByte,
    LoadHalfword,
    LoadSignedHalfword,
    LoadWord,
    LoadDoubleWord,

    StoreByte,
    StoreHalfword,
    StoreWord,
    StoreDoubleWord,
}

impl ArmLdrStrKind {
    pub fn is_ldr(self) -> bool {
        self < Self::StoreByte
    }

    pub fn is_str(self) -> bool {
        !self.is_ldr()
    }
}

pub struct ArmLdmStmConfig {
    pub pre: bool,
    pub up: bool,
    pub ldr: bool,
    pub writeback: bool,
}

pub const fn get_lut_table<I: ArmVisitor>() -> [fn(&mut I, ArmInst); 4096] {
    let mut lut: [fn(&mut I, ArmInst); 4096] = [I::arm_unknown_opcode; 4096];
    let mut i = 0;
    while i < 4096 {
        let as_inst = ((i & 0xFF0) << 16) | ((i & 0xF) >> 4);
        lut[i] = get_instruction_handler_inner::<I>(i, ArmInst(as_inst as u32), true);
        i += 1;
    }
    lut
}

#[bitmatch]
pub const fn get_instruction_handler<I: ArmVisitor>(
    i: ArmInst,
    for_lut: bool,
) -> fn(&mut I, ArmInst) {
    get_instruction_handler_inner(arm_inst_to_lookup_idx(i.0), i, for_lut)
}

#[bitmatch]
const fn get_instruction_handler_inner<I: ArmVisitor>(
    code: usize,
    i: ArmInst,
    for_lut: bool,
) -> fn(&mut I, ArmInst) {
    // Divided by GBATEK:
    #[bitmatch]
    match code {
        // Branch and Branch with Link (B, BL, BX, BLX, SWI, BKPT)
        // B
        "1010????_????" => |e, i| {
            let offs = RelativeOffset(i.0.i24() * 4);
            if I::IS_V5 && i.condition_code() == 0xF {
                e.arm_blx(ArmSignedOperandKind::Immediate(offs));
            } else {
                e.arm_b(offs)
            }
        },
        // BL
        "1011????_????" => |e, i| {
            let offs = RelativeOffset(i.0.i24() * 4);
            if I::IS_V5 && i.condition_code() == 0xF {
                e.arm_blx(ArmSignedOperandKind::Immediate(RelativeOffset(offs.0 + 2)));
            } else {
                e.arm_bl(offs)
            }
        },
        // BX / BLX
        "00010010_0001" if for_lut => |e, i| {
            if i.0.bits(8, 12) == 0b1111_1111_1111 {
                e.arm_bx(i.reg(0))
            } else {
                e.arm_unknown_opcode(i);
            }
        },
        "00010010_0010" if for_lut => |e, i| {
            if i.0.bits(8, 12) == 0b1111_1111_1111 {
                e.arm_bx(i.reg(0))
            } else {
                e.arm_unknown_opcode(i);
            }
        },
        "00010010_0011" if for_lut => |e, i| {
            if i.0.bits(8, 12) == 0b1111_1111_1111 {
                e.arm_blx(ArmSignedOperandKind::Register(i.reg(0)));
            } else {
                e.arm_unknown_opcode(i);
            }
        },
        "00010010_0001" if i.bits(8..=19) == 0b1111_1111_1111 => |e, i| e.arm_bx(i.reg(0)),
        "00010010_0010" if i.bits(8..=19) == 0b1111_1111_1111 => |e, i| e.arm_bx(i.reg(0)),
        "00010010_0011" if i.bits(8..=19) == 0b1111_1111_1111 => {
            |e, i| e.arm_blx(ArmSignedOperandKind::Register(i.reg(0)))
        }
        // SWI
        "1111????_????" => |e, _| e.arm_swi(),

        // Multiply and Multiply-Accumulate (MUL, MLA)
        // Word Multiples
        "0000000?_1001" => |e, i| {
            e.arm_mul::<{ ArmMulOp::Mul }>(i.reg(12), i.reg(8), i.reg(16), i.reg(0), i.is_bit(20))
        },
        "0000001?_1001" => |e, i| {
            e.arm_mul::<{ ArmMulOp::Mla }>(i.reg(12), i.reg(8), i.reg(16), i.reg(0), i.is_bit(20))
        },
        "00000100_1001" => {
            |e, i| e.arm_mul::<{ ArmMulOp::Umaal }>(i.reg(12), i.reg(8), i.reg(16), i.reg(0), false)
        }
        "0000100?_1001" => |e, i| {
            e.arm_mul::<{ ArmMulOp::Umull }>(i.reg(12), i.reg(8), i.reg(16), i.reg(0), i.is_bit(20))
        },
        "0000101?_1001" => |e, i| {
            e.arm_mul::<{ ArmMulOp::Umlal }>(i.reg(12), i.reg(8), i.reg(16), i.reg(0), i.is_bit(20))
        },
        "0000110?_1001" => |e, i| {
            e.arm_mul::<{ ArmMulOp::Smull }>(i.reg(12), i.reg(8), i.reg(16), i.reg(0), i.is_bit(20))
        },
        "0000111?_1001" => |e, i| {
            e.arm_mul::<{ ArmMulOp::Smlal }>(i.reg(12), i.reg(8), i.reg(16), i.reg(0), i.is_bit(20))
        },
        // Halfword Multiples
        "00010000_1??0" if I::IS_V5 => |e, i| {
            e.arm_sh_mul::<{ ArmShMulOp::SmlaXy }>(
                i.reg(12),
                i.reg(8),
                i.reg(16),
                i.reg(0),
                i.is_bit(5),
                i.is_bit(6),
            )
        },
        "00010010_1??0" if I::IS_V5 => |e, i| {
            e.arm_sh_mul::<{ ArmShMulOp::SmlawYOrSmulwY }>(
                i.reg(12),
                i.reg(8),
                i.reg(16),
                i.reg(0),
                i.is_bit(5),
                i.is_bit(6),
            )
        },
        "00010100_1??0" if I::IS_V5 => |e, i| {
            e.arm_sh_mul::<{ ArmShMulOp::SmlalXy }>(
                i.reg(12),
                i.reg(8),
                i.reg(16),
                i.reg(0),
                i.is_bit(5),
                i.is_bit(6),
            )
        },
        "00010110_1??0" if I::IS_V5 => |e, i| {
            e.arm_sh_mul::<{ ArmShMulOp::SmulXy }>(
                i.reg(12),
                i.reg(8),
                i.reg(16),
                i.reg(0),
                i.is_bit(5),
                i.is_bit(6),
            )
        },

        // Special ARM9 Instructions (CLZ, QADD/QSUB)
        // TODO

        // PSR Transfer (MRS, MSR)
        "00010000_0000" => |e, i| e.arm_mrs(i.reg(12), false),
        "00010100_0000" => |e, i| e.arm_mrs(i.reg(12), true),
        "00010010_0000" => |e, i| {
            e.arm_msr(
                ArmOperandKind::Register(i.reg(0)),
                i.is_bit(19),
                i.is_bit(16),
                false,
            )
        },
        "00010110_0000" => |e, i| {
            e.arm_msr(
                ArmOperandKind::Register(i.reg(0)),
                i.is_bit(19),
                i.is_bit(16),
                true,
            )
        },
        "00110010_????" => |e, i| {
            e.arm_msr(
                ArmOperandKind::Immediate((i.0 & 0xFF).rotate_right((i.0 >> 8) & 0xF) << 1),
                i.is_bit(19),
                i.is_bit(16),
                false,
            )
        },
        "00110110_????" => |e, i| {
            e.arm_msr(
                ArmOperandKind::Immediate((i.0 & 0xFF).rotate_right((i.0 >> 8) & 0xF) << 1),
                i.is_bit(19),
                i.is_bit(16),
                true,
            )
        },

        // Memory: Single Data Transfer (LDR, STR, PLD)
        // Immediate Offset
        "010??0?0_????" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Immediate(i.0 & 0xFFF),
                i.ldrstr_config(ArmLdrStrKind::StoreWord),
            )
        },
        "010??1?0_????" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Immediate(i.0 & 0xFFF),
                i.ldrstr_config(ArmLdrStrKind::StoreByte),
            )
        },
        "010??0?1_????" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Immediate(i.0 & 0xFFF),
                i.ldrstr_config(ArmLdrStrKind::LoadWord),
            )
        },
        "010??1?1_????" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Immediate(i.0 & 0xFFF),
                i.ldrstr_config(ArmLdrStrKind::LoadByte),
            )
        },
        // Register Offset
        "011??0?0_???0" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_shift_reg(),
                i.ldrstr_config(ArmLdrStrKind::StoreWord),
            )
        },
        "011??1?0_???0" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_shift_reg(),
                i.ldrstr_config(ArmLdrStrKind::StoreByte),
            )
        },
        "011??0?1_???0" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_shift_reg(),
                i.ldrstr_config(ArmLdrStrKind::LoadWord),
            )
        },
        "011??1?1_???0" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_shift_reg(),
                i.ldrstr_config(ArmLdrStrKind::LoadByte),
            )
        },

        // Memory: Halfword, Doubleword, and Signed Data Transfer
        // Immediate Offset
        "000??1?0_1011" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_split_imm(),
                i.ldrstr_config(ArmLdrStrKind::StoreHalfword),
            )
        },
        "000??1?0_1101" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_split_imm(),
                i.ldrstr_config(ArmLdrStrKind::LoadDoubleWord),
            )
        },
        "000??1?0_1111" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_split_imm(),
                i.ldrstr_config(ArmLdrStrKind::StoreDoubleWord),
            )
        },
        "000??1?1_1011" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_split_imm(),
                i.ldrstr_config(ArmLdrStrKind::LoadHalfword),
            )
        },
        "000??1?1_1101" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_split_imm(),
                i.ldrstr_config(ArmLdrStrKind::LoadSignedByte),
            )
        },
        "000??1?1_1111" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                i.ldrstr_split_imm(),
                i.ldrstr_config(ArmLdrStrKind::LoadSignedHalfword),
            )
        },
        // Register Offset
        "000??0?0_1011" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Register(i.reg(0)),
                i.ldrstr_config(ArmLdrStrKind::StoreHalfword),
            )
        },
        "000??0?0_1101" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Register(i.reg(0)),
                i.ldrstr_config(ArmLdrStrKind::LoadDoubleWord),
            )
        },
        "000??0?0_1111" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Register(i.reg(0)),
                i.ldrstr_config(ArmLdrStrKind::StoreDoubleWord),
            )
        },
        "000??0?1_1011" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Register(i.reg(0)),
                i.ldrstr_config(ArmLdrStrKind::LoadHalfword),
            )
        },
        "000??0?1_1101" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Register(i.reg(0)),
                i.ldrstr_config(ArmLdrStrKind::LoadSignedByte),
            )
        },
        "000??0?1_1111" => |e, i| {
            e.arm_ldrstr(
                i.reg(16),
                i.reg(12),
                ArmLdrStrOperandKind::Register(i.reg(0)),
                i.ldrstr_config(ArmLdrStrKind::LoadSignedHalfword),
            )
        },

        // Memory: Block Data Transfer (LDM, STM)
        "100?????_????" => |e, i| {
            e.arm_ldmstm(
                i.reg(16),
                i.0 as u16,
                i.is_bit(22),
                ArmLdmStmConfig {
                    pre: i.is_bit(24),
                    up: i.is_bit(23),
                    ldr: i.is_bit(20),
                    writeback: i.is_bit(21),
                },
            )
        },

        // Memory: Single Data Swap (SWP)
        "00010000_1001" => |e, i| e.arm_swp::<false>(i.reg(16), i.reg(12), i.reg(0)),
        "00010100_1001" => |e, i| e.arm_swp::<true>(i.reg(16), i.reg(12), i.reg(0)),

        // Data Processing (ALU)
        // With Register
        "000????0_???0" => |e, i| {
            e.arm_alu_reg::<false>(
                i.reg(16),
                i.reg(12),
                i.reg(0),
                i.safe_transmute(21..=24),
                i.safe_transmute(5..=6),
                ArmOperandKind::Immediate(i.bits(7..=11)),
            )
        },
        "000????1_???0" => |e, i| {
            e.arm_alu_reg::<true>(
                i.reg(16),
                i.reg(12),
                i.reg(0),
                i.safe_transmute(21..=24),
                i.safe_transmute(5..=6),
                ArmOperandKind::Immediate(i.bits(7..=11)),
            )
        },
        "000????0_0??1" => |e, i| {
            e.arm_alu_reg::<false>(
                i.reg(16),
                i.reg(12),
                i.reg(0),
                i.safe_transmute(21..=24),
                i.safe_transmute(5..=6),
                ArmOperandKind::Register(i.reg(8)),
            )
        },
        "000????1_0??1" => |e, i| {
            e.arm_alu_reg::<true>(
                i.reg(16),
                i.reg(12),
                i.reg(0),
                i.safe_transmute(21..=24),
                i.safe_transmute(5..=6),
                ArmOperandKind::Register(i.reg(8)),
            )
        },
        // With Immediate
        "001????0_????" => |e, i| {
            e.arm_alu_imm::<false>(
                i.reg(16),
                i.reg(12),
                (i.0 & 0xFF).rotate_right(i.bits(8..=11)),
                i.safe_transmute(21..=24),
            )
        },
        "001????1_????" => |e, i| {
            e.arm_alu_imm::<true>(
                i.reg(16),
                i.reg(12),
                (i.0 & 0xFF).rotate_right(i.bits(8..=11)),
                i.safe_transmute(21..=24),
            )
        },

        _ => I::arm_unknown_opcode,
    }
}

pub const fn arm_inst_to_lookup_idx(inst: u32) -> usize {
    ((inst as usize >> 16) & 0xFF0) | ((inst as usize >> 4) & 0xF)
}

#[cfg(test)]
mod test {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn decode_b() {
        disasm_ok((0b1010 << 24) | 0xFFF, "beq #0x3FFC");
        disasm_ok((0b1010 << 24) | 0xFFFFFF, "beq #-0x4");
    }

    #[test]
    fn decode_bl() {
        disasm_ok((0b1011 << 24) | 0xFFF, "bleq #0x3FFC");
        disasm_ok((0b1011 << 24) | 0xFFFFFF, "bleq #-0x4");
    }

    #[test]
    fn decode_bx() {
        disasm_ok(0b00010010111111111111_0001_1000, "bxeq r8");
        disasm_ok(0b00010010111111111111_0010_1001, "bxeq r9");
    }

    #[test]
    fn decode_blx() {
        disasm_ok((0b1111_1010 << 24) | 0xFFF, "blx #0x3FFC");
        disasm_ok((0b1111_1011 << 24) | 0xFFFFFF, "blx #-0x2");
        disasm_ok(0b00010010111111111111_0011_1110, "blxeq lr");
    }

    #[test]
    fn decode_swi() {
        disasm_ok(0b1111 << 24, "swieq");
        disasm_ok((0b1110_1111 << 24) | 0xFFF, "swi");
    }

    fn disasm_ok(asm: u32, disasm: &str) {
        let inst = ArmInst(asm);
        assert_eq!(disasm, inst.to_string())
    }
}
