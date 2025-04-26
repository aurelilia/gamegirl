// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use core::{fmt::UpperHex, marker::ConstParamTy};

use bitmatch::bitmatch;
use common::numutil::{NumExt, U16Ext};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::ThumbVisitor;
use crate::{
    memory::{Address, RelativeOffset},
    state::{LowRegister, Register},
};

#[derive(Copy, Clone)]
pub struct ThumbInst(u16);

impl ThumbInst {
    pub fn of(inst: u16) -> Self {
        Self(inst)
    }

    pub fn reg(self, idx: u16) -> LowRegister {
        LowRegister(self.0.bits(idx, 3))
    }

    pub fn reg16(self) -> (Register, Register) {
        (
            Register(self.0.bits(3, 4)),
            Register(self.reg(0).0 | (self.0.bit(7) << 3)),
        )
    }

    pub fn imm5(self) -> u32 {
        self.0.bits(6, 5).u32()
    }

    pub fn imm7(self) -> u32 {
        ((self.0 & 0x7F) << 2) as u32
    }

    pub fn imm8(self) -> u32 {
        (self.0 & 0xFF).u32()
    }

    pub fn imm10(self) -> i16 {
        self.0.i10()
    }

    pub fn imm11(self) -> u32 {
        self.0.bits(0, 11).u32() << 1
    }

    pub fn is_bit(self, bit: u16) -> bool {
        self.0.is_bit(bit)
    }

    pub fn thumb4(self) -> Thumb4Op {
        let o = self.0.bits(6, 4);
        Thumb4Op::from_u16(o).unwrap()
    }
}

impl UpperHex for ThumbInst {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        UpperHex::fmt(&self.0, f)
    }
}

#[derive(ConstParamTy, Debug, PartialEq, Eq)]
pub enum Thumb1Op {
    Lsl,
    Lsr,
    Asr,
    Add,
    Sub,
}

#[derive(ConstParamTy, Debug, PartialEq, Eq)]
pub enum Thumb2Op {
    Add,
    Sub,
}

#[derive(ConstParamTy, Debug, PartialEq, Eq)]
pub enum Thumb3Op {
    Mov,
    Cmp,
    Add,
    Sub,
}

#[derive(ConstParamTy, FromPrimitive, Debug, PartialEq, Eq)]
pub enum Thumb4Op {
    And = 0,
    Eor,
    Lsl,
    Lsr,
    Asr,
    Adc,
    Sbc,
    Ror,
    Tst,
    Neg,
    Cmp,
    Cmn,
    Orr,
    Mul,
    Bic,
    Mvn,
}

#[derive(ConstParamTy, Debug, PartialEq, Eq)]
pub enum ThumbStrLdrOp {
    Str = 0,
    Strh,
    Strb,
    Ldsb,
    Ldr,
    Ldrh,
    Ldrb,
    Ldsh,
}

pub const fn get_lut_table<I: ThumbVisitor>() -> [fn(&mut I, ThumbInst); 256] {
    let mut lut: [fn(&mut I, ThumbInst); 256] = [I::thumb_unknown_opcode; 256];
    let mut i = 0;
    while i < 256 {
        lut[i] = get_instruction_handler::<I>(ThumbInst((i << 8) as u16), true);
        i += 1;
    }
    lut
}

#[bitmatch]
pub const fn get_instruction_handler<I: ThumbVisitor>(
    i: ThumbInst,
    for_lut: bool,
) -> fn(&mut I, ThumbInst) {
    use Thumb1Op as Op1;
    use Thumb2Op as Op2;
    use Thumb3Op as Op3;
    use Thumb4Op as Op4;
    use ThumbStrLdrOp::*;

    #[bitmatch]
    match i.0 >> 6 {
        // THUMB.1/2
        "00000_?????" => |e, i| e.thumb_alu_imm::<{ Op1::Lsl }>(i.reg(0), i.reg(3), i.imm5()),
        "00001_?????" => |e, i| e.thumb_alu_imm::<{ Op1::Lsr }>(i.reg(0), i.reg(3), i.imm5()),
        "00010_?????" => |e, i| e.thumb_alu_imm::<{ Op1::Asr }>(i.reg(0), i.reg(3), i.imm5()),
        "0001110_???" => {
            |e, i| e.thumb_alu_imm::<{ Op1::Add }>(i.reg(0), i.reg(3), i.reg(6).0.u32())
        }
        "0001111_???" => {
            |e, i| e.thumb_alu_imm::<{ Op1::Sub }>(i.reg(0), i.reg(3), i.reg(6).0.u32())
        }
        "0001100_???" => |e, i| e.thumb_2_reg::<{ Op2::Add }>(i.reg(0), i.reg(3), i.reg(6)),
        "0001101_???" => |e, i| e.thumb_2_reg::<{ Op2::Sub }>(i.reg(0), i.reg(3), i.reg(6)),

        // THUMB.3
        "00100_?????" => |e, i| e.thumb_3::<{ Op3::Mov }>(i.reg(8), i.imm8()),
        "00101_?????" => |e, i| e.thumb_3::<{ Op3::Cmp }>(i.reg(8), i.imm8()),
        "00110_?????" => |e, i| e.thumb_3::<{ Op3::Add }>(i.reg(8), i.imm8()),
        "00111_?????" => |e, i| e.thumb_3::<{ Op3::Sub }>(i.reg(8), i.imm8()),

        // THUMB.4
        "010000_????" if for_lut => |e, i| match i.thumb4() {
            Op4::And => e.thumb_alu::<{ Op4::And }>(i.reg(0), i.reg(3)),
            Op4::Eor => e.thumb_alu::<{ Op4::Eor }>(i.reg(0), i.reg(3)),
            Op4::Lsl => e.thumb_alu::<{ Op4::Lsl }>(i.reg(0), i.reg(3)),
            Op4::Lsr => e.thumb_alu::<{ Op4::Lsr }>(i.reg(0), i.reg(3)),
            Op4::Asr => e.thumb_alu::<{ Op4::Asr }>(i.reg(0), i.reg(3)),
            Op4::Adc => e.thumb_alu::<{ Op4::Adc }>(i.reg(0), i.reg(3)),
            Op4::Sbc => e.thumb_alu::<{ Op4::Sbc }>(i.reg(0), i.reg(3)),
            Op4::Ror => e.thumb_alu::<{ Op4::Ror }>(i.reg(0), i.reg(3)),
            Op4::Tst => e.thumb_alu::<{ Op4::Tst }>(i.reg(0), i.reg(3)),
            Op4::Neg => e.thumb_alu::<{ Op4::Neg }>(i.reg(0), i.reg(3)),
            Op4::Cmp => e.thumb_alu::<{ Op4::Cmp }>(i.reg(0), i.reg(3)),
            Op4::Cmn => e.thumb_alu::<{ Op4::Cmn }>(i.reg(0), i.reg(3)),
            Op4::Orr => e.thumb_alu::<{ Op4::Orr }>(i.reg(0), i.reg(3)),
            Op4::Mul => e.thumb_alu::<{ Op4::Mul }>(i.reg(0), i.reg(3)),
            Op4::Bic => e.thumb_alu::<{ Op4::Bic }>(i.reg(0), i.reg(3)),
            Op4::Mvn => e.thumb_alu::<{ Op4::Mvn }>(i.reg(0), i.reg(3)),
        },
        "0100000000" if !for_lut => |e, i| e.thumb_alu::<{ Op4::And }>(i.reg(0), i.reg(3)),
        "0100000001" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Eor }>(i.reg(0), i.reg(3)),
        "0100000010" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Lsl }>(i.reg(0), i.reg(3)),
        "0100000011" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Lsr }>(i.reg(0), i.reg(3)),
        "0100000100" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Asr }>(i.reg(0), i.reg(3)),
        "0100000101" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Adc }>(i.reg(0), i.reg(3)),
        "0100000110" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Sbc }>(i.reg(0), i.reg(3)),
        "0100000111" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Ror }>(i.reg(0), i.reg(3)),
        "0100001000" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Tst }>(i.reg(0), i.reg(3)),
        "0100001001" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Neg }>(i.reg(0), i.reg(3)),
        "0100001010" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Cmp }>(i.reg(0), i.reg(3)),
        "0100001011" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Cmn }>(i.reg(0), i.reg(3)),
        "0100001100" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Orr }>(i.reg(0), i.reg(3)),
        "0100001101" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Mul }>(i.reg(0), i.reg(3)),
        "0100001110" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Bic }>(i.reg(0), i.reg(3)),
        "0100001111" if !for_lut => |e, i| e.thumb_alu::<{ Op4::Mvn }>(i.reg(0), i.reg(3)),

        // THUMB.5
        "01000100_??" => |e, i| e.thumb_hi_add(i.reg16()),
        "01000101_??" => |e, i| e.thumb_hi_cmp(i.reg16()),
        "01000110_??" => |e, i| e.thumb_hi_mov(i.reg16()),
        "01000111_??" => |e, i| {
            let (s, d) = i.reg16();
            e.thumb_hi_bx(s, d.0 > 7)
        },

        // THUMB.6
        "01001_?????" => |e, i| e.thumb_ldr6(i.reg(8), Address(i.imm8().u32() << 2)),
        // THUMB.7/8
        "0101000_???" => |e, i| e.thumb_ldrstr78::<{ Str }>(i.reg(0), i.reg(3), i.reg(6)),
        "0101001_???" => |e, i| e.thumb_ldrstr78::<{ Strh }>(i.reg(0), i.reg(3), i.reg(6)),
        "0101010_???" => |e, i| e.thumb_ldrstr78::<{ Strb }>(i.reg(0), i.reg(3), i.reg(6)),
        "0101011_???" => |e, i| e.thumb_ldrstr78::<{ Ldsb }>(i.reg(0), i.reg(3), i.reg(6)),
        "0101100_???" => |e, i| e.thumb_ldrstr78::<{ Ldr }>(i.reg(0), i.reg(3), i.reg(6)),
        "0101101_???" => |e, i| e.thumb_ldrstr78::<{ Ldrh }>(i.reg(0), i.reg(3), i.reg(6)),
        "0101110_???" => |e, i| e.thumb_ldrstr78::<{ Ldrb }>(i.reg(0), i.reg(3), i.reg(6)),
        "0101111_???" => |e, i| e.thumb_ldrstr78::<{ Ldsh }>(i.reg(0), i.reg(3), i.reg(6)),
        // THUMB.9
        "01100_?????" => {
            |e, i| e.thumb_ldrstr9::<{ Str }>(i.reg(0), i.reg(3), Address(i.imm5() << 2))
        }
        "01101_?????" => {
            |e, i| e.thumb_ldrstr9::<{ Ldr }>(i.reg(0), i.reg(3), Address(i.imm5() << 2))
        }
        "01110_?????" => |e, i| e.thumb_ldrstr9::<{ Strb }>(i.reg(0), i.reg(3), Address(i.imm5())),
        "01111_?????" => |e, i| e.thumb_ldrstr9::<{ Ldrb }>(i.reg(0), i.reg(3), Address(i.imm5())),
        // THUMB.10
        "10000_?????" => {
            |e, i| e.thumb_ldrstr10::<true>(i.reg(0), i.reg(3), Address(i.imm5() << 1))
        }
        "10001_?????" => {
            |e, i| e.thumb_ldrstr10::<false>(i.reg(0), i.reg(3), Address(i.imm5() << 1))
        }

        // THUMB.11
        "10010_?????" => |e, i| e.thumb_str_sp(i.reg(8), Address(i.imm8() << 2)),
        "10011_?????" => |e, i| e.thumb_ldr_sp(i.reg(8), Address(i.imm8() << 2)),

        // THUMB.12
        "10100_?????" => |e, i| e.thumb_rel_addr::<false>(i.reg(8), Address(i.imm8() << 2)),
        "10101_?????" => |e, i| e.thumb_rel_addr::<true>(i.reg(8), Address(i.imm8() << 2)),

        // THUMB.13
        "10110000_??" => |e, i| {
            let offset = i.imm7() as i32;
            e.thumb_sp_offs(RelativeOffset(if i.is_bit(7) { -offset } else { offset }))
        },

        // THUMB.14
        "10110100_??" => |e, i| e.thumb_push(i.0 as u8, false),
        "10110101_??" => |e, i| e.thumb_push(i.0 as u8, true),
        "10111100_??" => |e, i| e.thumb_pop(i.0 as u8, false),
        "10111101_??" => |e, i| e.thumb_pop(i.0 as u8, true),

        // THUMB.15
        "11000_?????" => |e, i| e.thumb_stmia(i.reg(8), i.0 as u8),
        "11001_?????" => |e, i| e.thumb_ldmia(i.reg(8), i.0 as u8),

        // THUMB.16/17
        "11011111_??" => |e, _| e.thumb_swi(),
        "1101_??????" => |e, i| {
            e.thumb_bcond(
                (i.0 >> 8) & 0xF,
                RelativeOffset((i.imm8() as i8 as i32) * 2),
            )
        },

        // THUMB.18
        "11100_?????" => |e, i| e.thumb_br(RelativeOffset(i.imm10() as i32 * 2)),
        // THUMB.19
        "11110_?????" => |e, i| e.thumb_set_lr(RelativeOffset((i.imm10() as i32) << 12)),
        "11101_?????" => |e, i| e.thumb_bl::<false>(Address(i.imm11())),
        "11111_?????" => |e, i| e.thumb_bl::<true>(Address(i.imm11())),

        _ => I::thumb_unknown_opcode,
    }
}

#[cfg(test)]
mod test {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn decode_thumb1() {
        disasm_ok(0b000_00_11111_101_001, "lsl r1, r5, $31");
        disasm_ok(0b000_01_10000_101_011, "lsr r3, r5, $16");
        disasm_ok(0b000_10_00010_101_000, "asr r0, r5, $2");
    }

    #[test]
    fn decode_thumb2() {
        disasm_ok(0b00011_00_000_101_001, "add r1, r5, r0");
        disasm_ok(0b00011_01_000_101_001, "sub r1, r5, r0");
        disasm_ok(0b00011_10_010_101_011, "add r3, r5, $2");
        disasm_ok(0b00011_11_100_101_001, "sub r1, r5, $4");
    }

    #[test]
    fn decode_thumb3() {
        disasm_ok(0b001_00_100_11111111, "mov r4, $0xFF");
        disasm_ok(0b001_01_110_01111111, "cmp r6, $0x7F");
        disasm_ok(0b001_10_111_00111111, "add r7, $0x3F");
        disasm_ok(0b001_11_001_11110111, "sub r1, $0xF7");
    }

    #[test]
    fn decode_thumb4() {
        disasm_ok(0b010000_0000_111_111, "and r7, r7");
        disasm_ok(0b010000_0001_111_110, "eor r6, r7");
        disasm_ok(0b010000_0010_111_110, "lsl r6, r7");
        disasm_ok(0b010000_0011_111_110, "lsr r6, r7");
        disasm_ok(0b010000_0100_111_110, "asr r6, r7");
        disasm_ok(0b010000_0101_111_110, "adc r6, r7");
        disasm_ok(0b010000_0110_111_110, "sbc r6, r7");
        disasm_ok(0b010000_0111_111_110, "ror r6, r7");
        disasm_ok(0b010000_1000_111_110, "tst r7");
        disasm_ok(0b010000_1001_111_110, "neg r6, r7");
        disasm_ok(0b010000_1010_111_110, "cmp r6, r7");
        disasm_ok(0b010000_1011_111_110, "cmn r6, r7");
        disasm_ok(0b010000_1100_111_110, "orr r6, r7");
        disasm_ok(0b010000_1101_111_111, "mul r7, r7");
        disasm_ok(0b010000_1110_111_111, "bic r7, r7");
        disasm_ok(0b010000_1111_111_111, "mvn r7, r7");
    }

    #[test]
    fn decode_thumb5() {
        disasm_ok(0b010001_00_11_111_111, "add pc, pc");
        disasm_ok(0b010001_01_00_111_111, "cmp r7, r7");
        disasm_ok(0b010001_10_10_110_110, "mov lr, r6");
        disasm_ok(0b010001_11_00_110_110, "bx r6");
        disasm_ok(0b010001_11_11_011_110, "blx r11");
    }

    #[test]
    fn decode_thumb6() {
        disasm_ok(0b01001_100_11111111, "ldr r4, [PC, $0x3FC]");
    }

    #[test]
    fn decode_thumb7() {
        disasm_ok(0b0101_00_0_001_010_100, "str r4, [r2, r1]");
        disasm_ok(0b0101_01_0_001_010_100, "strb r4, [r2, r1]");
        disasm_ok(0b0101_10_0_001_010_100, "ldr r4, [r2, r1]");
        disasm_ok(0b0101_11_0_001_010_100, "ldrb r4, [r2, r1]");
    }

    #[test]
    fn decode_thumb8() {
        disasm_ok(0b011_00_11111_110_101, "str r5, [r6, $0x7C]");
        disasm_ok(0b0101_01_1_001_010_100, "ldsb r4, [r2, r1]");
        disasm_ok(0b0101_10_1_001_010_100, "ldrh r4, [r2, r1]");
        disasm_ok(0b0101_11_1_001_010_100, "ldsh r4, [r2, r1]");
    }

    #[test]
    fn decode_thumb9() {
        disasm_ok(0b011_00_11111_110_101, "str r5, [r6, $0x7C]");
        disasm_ok(0b011_01_00001_110_101, "ldr r5, [r6, $0x4]");
        disasm_ok(0b011_10_11111_110_101, "strb r5, [r6, $0x1F]");
        disasm_ok(0b011_11_00001_110_101, "ldrb r5, [r6, $0x1]");
    }

    #[test]
    fn decode_thumb10() {
        disasm_ok(0b1000_0_00001_001_000, "strh r0, [r1, $0x2]");
        disasm_ok(0b1000_1_00001_001_000, "ldrh r0, [r1, $0x2]");
    }

    #[test]
    fn decode_thumb11() {
        disasm_ok(0b1001_0_100_00000001, "str r4, [sp, $0x4]");
        disasm_ok(0b1001_1_100_00000001, "ldr r4, [sp, $0x4]");
    }

    #[test]
    fn decode_thumb12() {
        disasm_ok(0b1010_0_000_00000010, "add r0, pc, $0x8");
        disasm_ok(0b1010_1_000_00000011, "add r0, sp, $0xC");
    }

    #[test]
    fn decode_thumb13() {
        disasm_ok(0b10110000_0_0000010, "add sp, $0x8");
        disasm_ok(0b10110000_1_0000100, "add sp, $-0x10");
    }

    #[test]
    fn decode_thumb14() {
        disasm_ok(0b1011_0_10_0_11000000, "push r6 r7");
        disasm_ok(0b1011_1_10_0_00110000, "pop r4 r5");
        disasm_ok(0b1011_0_10_1_11000000, "push r6 r7 lr");
        disasm_ok(0b1011_1_10_1_00110000, "pop r4 r5 pc");
    }

    #[test]
    fn decode_thumb15() {
        disasm_ok(0b1100_0_001_11000000, "stmia r1!, r6 r7");
        disasm_ok(0b1100_1_011_00000011, "ldmia r3!, r0 r1");
    }

    #[test]
    fn decode_thumb16() {
        disasm_ok(0b1101_0000_11111111, "beq $-0x2");
        disasm_ok(0b1101_1000_01111111, "bhi $0xFE");
    }

    #[test]
    fn decode_thumb17() {
        disasm_ok(0b11011111_00000000, "swi");
    }

    #[test]
    fn decode_thumb18() {
        disasm_ok(0b11100_11111111111, "b $-0x2");
        disasm_ok(0b11100_00000000001, "b $0x2");
    }

    #[test]
    fn decode_thumb19() {
        disasm_ok(0b11110_00000000010, "mov lr, (pc + $0x2000)");
        disasm_ok(0b11111_00000000010, "bl lr + $0x4");
        disasm_ok(0b11101_00000000010, "blx lr + $0x4");
    }

    fn disasm_ok(asm: u16, disasm: &str) {
        let inst = ThumbInst(asm);
        assert_eq!(disasm, inst.to_string())
    }
}
