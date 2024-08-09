// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{fmt::Display, marker::ConstParamTy};

use bitmatch::bitmatch;
use common::numutil::{NumExt, U16Ext};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{ThumbExecutor, ThumbLut};
use crate::misc;

#[derive(Copy, Clone)]
pub struct ThumbInst(u16);

impl ThumbInst {
    pub fn of(inst: u16) -> Self {
        Self(inst)
    }

    pub fn reg(self, idx: u16) -> u16 {
        self.0.bits(idx, 3)
    }

    pub fn reg16(self) -> (u16, u16) {
        (self.0.bits(3, 4), self.reg(0) | (self.0.bit(7) << 3))
    }

    pub fn imm5(self) -> u16 {
        self.0.bits(6, 5)
    }

    pub fn imm7(self) -> u16 {
        (self.0 & 0x7F) << 2
    }

    pub fn imm8(self) -> u16 {
        self.0 & 0xFF
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

#[derive(ConstParamTy, PartialEq, Eq)]
pub enum Thumb12Op {
    Lsl,
    Lsr,
    Asr,
    AddReg,
    SubReg,
    AddImm,
    SubImm,
}

#[derive(ConstParamTy, PartialEq, Eq)]
pub enum Thumb3Op {
    Mov,
    Cmp,
    Add,
    Sub,
}

#[derive(FromPrimitive, PartialEq, Eq)]
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

#[derive(ConstParamTy, PartialEq, Eq)]
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

#[allow(clippy::unreadable_literal)]
#[allow(private_bounds)]
pub const fn make_thumb_lut<I: ThumbExecutor>() -> ThumbLut<I> {
    use Thumb12Op::*;
    use Thumb3Op::*;
    use ThumbStrLdrOp::*;

    let mut lut: ThumbLut<I> = [I::thumb_unknown_opcode; 256];

    lut[0b1101_1111] = I::thumb_swi;
    lut[0b1011_0000] = I::thumb_sp_offs;

    lut[0b0100_0100] = I::thumb_hi_add;
    lut[0b0100_0101] = I::thumb_hi_cmp;
    lut[0b0100_0110] = I::thumb_hi_mov;
    lut[0b0100_0111] = I::thumb_hi_bx;

    lut_span(&mut lut, 0b00000, 5, I::thumb_arithmetic::<{ Lsl }>);
    lut_span(&mut lut, 0b00001, 5, I::thumb_arithmetic::<{ Lsr }>);
    lut_span(&mut lut, 0b00010, 5, I::thumb_arithmetic::<{ Asr }>);
    lut_span(&mut lut, 0b0001100, 7, I::thumb_arithmetic::<{ AddReg }>);
    lut_span(&mut lut, 0b0001101, 7, I::thumb_arithmetic::<{ SubReg }>);
    lut_span(&mut lut, 0b0001110, 7, I::thumb_arithmetic::<{ AddImm }>);
    lut_span(&mut lut, 0b0001111, 7, I::thumb_arithmetic::<{ SubImm }>);

    lut_span(&mut lut, 0b00100, 5, I::thumb_3::<{ Mov }>);
    lut_span(&mut lut, 0b00101, 5, I::thumb_3::<{ Cmp }>);
    lut_span(&mut lut, 0b00110, 5, I::thumb_3::<{ Add }>);
    lut_span(&mut lut, 0b00111, 5, I::thumb_3::<{ Sub }>);

    lut_span(&mut lut, 0b010000, 6, I::thumb_alu);
    lut_span(&mut lut, 0b01001, 5, I::thumb_ldr6);

    lut_span(&mut lut, 0b0101000, 7, I::thumb_ldrstr78::<{ Str }>);
    lut_span(&mut lut, 0b0101001, 7, I::thumb_ldrstr78::<{ Strh }>);
    lut_span(&mut lut, 0b0101010, 7, I::thumb_ldrstr78::<{ Strb }>);
    lut_span(&mut lut, 0b0101011, 7, I::thumb_ldrstr78::<{ Ldsb }>);
    lut_span(&mut lut, 0b0101100, 7, I::thumb_ldrstr78::<{ Ldr }>);
    lut_span(&mut lut, 0b0101101, 7, I::thumb_ldrstr78::<{ Ldrh }>);
    lut_span(&mut lut, 0b0101110, 7, I::thumb_ldrstr78::<{ Ldrb }>);
    lut_span(&mut lut, 0b0101111, 7, I::thumb_ldrstr78::<{ Ldsh }>);

    lut_span(&mut lut, 0b01100, 5, I::thumb_ldrstr9::<{ Str }>);
    lut_span(&mut lut, 0b01101, 5, I::thumb_ldrstr9::<{ Ldr }>);
    lut_span(&mut lut, 0b01110, 5, I::thumb_ldrstr9::<{ Strb }>);
    lut_span(&mut lut, 0b01111, 5, I::thumb_ldrstr9::<{ Ldrb }>);
    lut_span(&mut lut, 0b10000, 5, I::thumb_ldrstr10::<true>);
    lut_span(&mut lut, 0b10001, 5, I::thumb_ldrstr10::<false>);
    lut_span(&mut lut, 0b10010, 5, I::thumb_str_sp);
    lut_span(&mut lut, 0b10011, 5, I::thumb_ldr_sp);

    lut_span(&mut lut, 0b10100, 5, I::thumb_rel_addr::<false>);
    lut_span(&mut lut, 0b10101, 5, I::thumb_rel_addr::<true>);

    lut[0b1011_0100] = I::thumb_push::<false>;
    lut[0b1011_0101] = I::thumb_push::<true>;
    lut[0b1011_1100] = I::thumb_pop::<false>;
    lut[0b1011_1101] = I::thumb_pop::<true>;
    lut_span(&mut lut, 0b11000, 5, I::thumb_stmia);
    lut_span(&mut lut, 0b11001, 5, I::thumb_ldmia);

    // Ugh.
    lut[0xD0] = I::thumb_bcond::<0x0>;
    lut[0xD1] = I::thumb_bcond::<0x1>;
    lut[0xD2] = I::thumb_bcond::<0x2>;
    lut[0xD3] = I::thumb_bcond::<0x3>;
    lut[0xD4] = I::thumb_bcond::<0x4>;
    lut[0xD5] = I::thumb_bcond::<0x5>;
    lut[0xD6] = I::thumb_bcond::<0x6>;
    lut[0xD7] = I::thumb_bcond::<0x7>;
    lut[0xD8] = I::thumb_bcond::<0x8>;
    lut[0xD9] = I::thumb_bcond::<0x9>;
    lut[0xDA] = I::thumb_bcond::<0xA>;
    lut[0xDB] = I::thumb_bcond::<0xB>;
    lut[0xDC] = I::thumb_bcond::<0xC>;
    lut[0xDD] = I::thumb_bcond::<0xD>;
    lut[0xDE] = I::thumb_bcond::<0xE>;

    lut_span(&mut lut, 0b11100, 5, I::thumb_br);
    lut_span(&mut lut, 0b11110, 5, I::thumb_set_lr);
    lut_span(&mut lut, 0b11101, 5, I::thumb_bl::<false>);
    lut_span(&mut lut, 0b11111, 5, I::thumb_bl::<true>);

    lut
}

pub const fn lut_span<T: Copy>(lut: &mut [T], idx: usize, size: usize, handler: T) {
    let inst = 8 - size;
    let start = idx << inst;

    let until = 1 << inst;
    let mut idx = 0;
    while idx < until {
        lut[start | idx] = handler;
        idx += 1;
    }
}

impl Display for ThumbInst {
    #[bitmatch]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[bitmatch]
        match self.0 {
            "11011111_nnnnnnnn" => write!(f, "swi 0x{:02X}", n),

            "000_00nnnnnsssddd" => write!(f, "lsl r{d}, r{s}, #0x{:X}", n),
            "000_01nnnnnsssddd" => write!(f, "lsr r{d}, r{s}, #0x{:X}", n),
            "000_10nnnnnsssddd" => write!(f, "asr r{d}, r{s}, #0x{:X}", n),
            "00011_00nnnsssddd" => write!(f, "add r{d}, r{s}, r{n}"),
            "00011_01nnnsssddd" => write!(f, "sub r{d}, r{s}, r{n}"),
            "00011_10nnnsssddd" => write!(f, "add r{d}, r{s}, #0x{:X}", n),
            "00011_11nnnsssddd" => write!(f, "sub r{d}, r{s}, #0x{:X}", n),

            "001_00dddnnnnnnnn" => write!(f, "mov r{d}, #{n}"),
            "001_01dddnnnnnnnn" => write!(f, "cmp r{d}, #{n}"),
            "001_10dddnnnnnnnn" => write!(f, "add r{d}, #{n}"),
            "001_11dddnnnnnnnn" => write!(f, "sub r{d}, #{n}"),

            "010000_oooosssddd" => {
                let op = match o {
                    0x0 => "and",
                    0x1 => "eor",
                    0x2 => "lsl",
                    0x3 => "lsr",
                    0x4 => "asr",
                    0x5 => "add",
                    0x6 => "sub",
                    0x7 => "ror",
                    0x8 => "tst",
                    0x9 => "neg",
                    0xA => "cmp",
                    0xB => "cmn",
                    0xC => "orr",
                    0xD => "mul",
                    0xE => "bic",
                    _ => "mvn",
                };
                if o == 0x8 {
                    write!(f, "{op} r{s}")
                } else {
                    write!(f, "{op} r{d}, r{s}")
                }
            }

            "010001_00dssssddd" => write!(f, "add r{d}, r{s}"),
            "010001_01dssssddd" => write!(f, "cmp r{d}, r{s}"),
            "010001_10dssssddd" => write!(f, "mov r{d}, r{s}"),
            "010001_110ssss???" => write!(f, "bx r{s}"),
            "010001_111ssss???" => write!(f, "blx r{s}"),
            "01001_dddnnnnnnnn" => write!(f, "ldr r{d}, [PC, #0x{:X}]", (n.u32() << 2)),
            "0101_ooosssbbbddd" => {
                let op = match o {
                    0 => "str",
                    1 => "strh",
                    2 => "strb",
                    3 => "ldsb",
                    4 => "ldr",
                    5 => "ldrh",
                    6 => "ldrb",
                    _ => "ldsh",
                };
                write!(f, "{op} r{d}, [r{b}, r{s}]")
            }
            "011_oonnnnnbbbddd" => {
                let op = match o {
                    0 => "str",
                    1 => "ldr",
                    2 => "strb",
                    _ => "ldrb",
                };
                write!(f, "{op} r{d}, [r{b}, #0x{:X}]", n)
            }
            "1000_0nnnnnbbbddd" => write!(f, "strh r{d}, [r{b}, #0x{:X}]", n << 1),
            "1000_1nnnnnbbbddd" => write!(f, "ldrh r{d}, [r{b}, #0x{:X}]", n << 1),
            "1001_0dddnnnnnnnn" => write!(f, "str r{d}, [sp, #0x{:X}]", n << 2),
            "1001_1dddnnnnnnnn" => write!(f, "ldr r{d}, [sp, #0x{:X}]", n << 2),

            "1010_0dddnnnnnnnn" => write!(f, "add r{d}, pc, #0x{:X}", n << 2),
            "1010_1dddnnnnnnnn" => write!(f, "add r{d}, sp, #0x{:X}", n << 2),

            "10110000_0nnnnnnn" => write!(f, "add sp, #0x{:X}", n << 2),
            "10110000_1nnnnnnn" => write!(f, "add sp, #-0x{:X}", n << 2),

            "1011_0100rrrrrrrr" => write!(f, "push {:08b}", r),
            "1011_0101rrrrrrrr" => write!(f, "push {:08b}, lr", r),
            "1011_1100rrrrrrrr" => write!(f, "pop {:08b}", r),
            "1011_1101rrrrrrrr" => write!(f, "pop {:08b}, pc", r),
            "1100_0bbbrrrrrrrr" => write!(f, "stmia r{b}!, {:08b}", r),
            "1100_1bbbrrrrrrrr" => write!(f, "ldmia r{b}!, {:08b}", r),

            "1101_ccccnnnnnnnn" => write!(
                f,
                "b{} 0x{:X}",
                misc::condition_mnemonic(c).to_ascii_lowercase(),
                ((n as i8 as i16) * 2) + 2
            ),
            "11100_nnnnnnnnnnn" => write!(f, "b 0x{:X}", (n.i10() << 1) + 2),
            "11110_nnnnnnnnnnn" => write!(f, "mov lr, (pc + 0x{:X})", n << 12),
            "11111_nnnnnnnnnnn" => write!(f, "bl lr + 0x{:X}", n << 1),
            "11101_nnnnnnnnnnn" => write!(f, "blx lr + 0x{:X}", n << 1),

            _ => write!(f, "{:04X}??", self.0),
        }
    }
}
