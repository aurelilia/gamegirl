// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use super::{
    super::interface::{ArmSystem, SysWrapper},
    ArmLut,
};

impl<S: ArmSystem> SysWrapper<S> {
    #[allow(clippy::unusual_byte_groupings)]
    pub const fn make_arm_lut() -> ArmLut<S> {
        let mut lut: ArmLut<S> = [Self::arm_unknown_opcode; 256];

        Self::lut_span(&mut lut, 0b1010, 4, Self::arm_b::<false>);
        Self::lut_span(&mut lut, 0b1011, 4, Self::arm_b::<true>);
        Self::lut_span(&mut lut, 0b1111, 4, Self::arm_swi);

        // Ew.
        lut[0b000_0000_0] = Self::arm_alu_mul_psr_reg::<0b0000, false>;
        lut[0b000_0001_0] = Self::arm_alu_mul_psr_reg::<0b0001, false>;
        lut[0b000_0010_0] = Self::arm_alu_mul_psr_reg::<0b0010, false>;
        lut[0b000_0011_0] = Self::arm_alu_mul_psr_reg::<0b0011, false>;
        lut[0b000_0100_0] = Self::arm_alu_mul_psr_reg::<0b0100, false>;
        lut[0b000_0101_0] = Self::arm_alu_mul_psr_reg::<0b0101, false>;
        lut[0b000_0110_0] = Self::arm_alu_mul_psr_reg::<0b0110, false>;
        lut[0b000_0111_0] = Self::arm_alu_mul_psr_reg::<0b0111, false>;
        lut[0b000_1000_0] = Self::arm_alu_gap::<0b1000>;
        lut[0b000_1001_0] = Self::arm_alu_gap::<0b1001>;
        lut[0b000_1010_0] = Self::arm_alu_gap::<0b1010>;
        lut[0b000_1011_0] = Self::arm_alu_gap::<0b1011>;
        lut[0b000_1100_0] = Self::arm_alu_mul_psr_reg::<0b1100, false>;
        lut[0b000_1101_0] = Self::arm_alu_mul_psr_reg::<0b1101, false>;
        lut[0b000_1110_0] = Self::arm_alu_mul_psr_reg::<0b1110, false>;
        lut[0b000_1111_0] = Self::arm_alu_mul_psr_reg::<0b1111, false>;
        lut[0b000_0000_1] = Self::arm_alu_mul_psr_reg::<0b0000, true>;
        lut[0b000_0001_1] = Self::arm_alu_mul_psr_reg::<0b0001, true>;
        lut[0b000_0010_1] = Self::arm_alu_mul_psr_reg::<0b0010, true>;
        lut[0b000_0011_1] = Self::arm_alu_mul_psr_reg::<0b0011, true>;
        lut[0b000_0100_1] = Self::arm_alu_mul_psr_reg::<0b0100, true>;
        lut[0b000_0101_1] = Self::arm_alu_mul_psr_reg::<0b0101, true>;
        lut[0b000_0110_1] = Self::arm_alu_mul_psr_reg::<0b0110, true>;
        lut[0b000_0111_1] = Self::arm_alu_mul_psr_reg::<0b0111, true>;
        lut[0b000_1000_1] = Self::arm_alu_mul_psr_reg::<0b1000, true>;
        lut[0b000_1001_1] = Self::arm_alu_mul_psr_reg::<0b1001, true>;
        lut[0b000_1010_1] = Self::arm_alu_mul_psr_reg::<0b1010, true>;
        lut[0b000_1011_1] = Self::arm_alu_mul_psr_reg::<0b1011, true>;
        lut[0b000_1100_1] = Self::arm_alu_mul_psr_reg::<0b1100, true>;
        lut[0b000_1101_1] = Self::arm_alu_mul_psr_reg::<0b1101, true>;
        lut[0b000_1110_1] = Self::arm_alu_mul_psr_reg::<0b1110, true>;
        lut[0b000_1111_1] = Self::arm_alu_mul_psr_reg::<0b1111, true>;
        lut[0b001_0000_0] = Self::arm_alu_imm::<0b0000, false>;
        lut[0b001_0001_0] = Self::arm_alu_imm::<0b0001, false>;
        lut[0b001_0010_0] = Self::arm_alu_imm::<0b0010, false>;
        lut[0b001_0011_0] = Self::arm_alu_imm::<0b0011, false>;
        lut[0b001_0100_0] = Self::arm_alu_imm::<0b0100, false>;
        lut[0b001_0101_0] = Self::arm_alu_imm::<0b0101, false>;
        lut[0b001_0110_0] = Self::arm_alu_imm::<0b0110, false>;
        lut[0b001_0111_0] = Self::arm_alu_imm::<0b0111, false>;
        lut[0b001_1000_0] = Self::arm_alu_imm::<0b1000, false>;
        lut[0b001_1001_0] = Self::arm_alu_imm::<0b1001, false>;
        lut[0b001_1010_0] = Self::arm_alu_imm::<0b1010, false>;
        lut[0b001_1011_0] = Self::arm_alu_imm::<0b1011, false>;
        lut[0b001_1100_0] = Self::arm_alu_imm::<0b1100, false>;
        lut[0b001_1101_0] = Self::arm_alu_imm::<0b1101, false>;
        lut[0b001_1110_0] = Self::arm_alu_imm::<0b1110, false>;
        lut[0b001_1111_0] = Self::arm_alu_imm::<0b1111, false>;
        lut[0b001_0000_1] = Self::arm_alu_imm::<0b0000, true>;
        lut[0b001_0001_1] = Self::arm_alu_imm::<0b0001, true>;
        lut[0b001_0010_1] = Self::arm_alu_imm::<0b0010, true>;
        lut[0b001_0011_1] = Self::arm_alu_imm::<0b0011, true>;
        lut[0b001_0100_1] = Self::arm_alu_imm::<0b0100, true>;
        lut[0b001_0101_1] = Self::arm_alu_imm::<0b0101, true>;
        lut[0b001_0110_1] = Self::arm_alu_imm::<0b0110, true>;
        lut[0b001_0111_1] = Self::arm_alu_imm::<0b0111, true>;
        lut[0b001_1000_1] = Self::arm_alu_imm::<0b1000, true>;
        lut[0b001_1001_1] = Self::arm_alu_imm::<0b1001, true>;
        lut[0b001_1010_1] = Self::arm_alu_imm::<0b1010, true>;
        lut[0b001_1011_1] = Self::arm_alu_imm::<0b1011, true>;
        lut[0b001_1100_1] = Self::arm_alu_imm::<0b1100, true>;
        lut[0b001_1101_1] = Self::arm_alu_imm::<0b1101, true>;
        lut[0b001_1110_1] = Self::arm_alu_imm::<0b1110, true>;
        lut[0b001_1111_1] = Self::arm_alu_imm::<0b1111, true>;

        lut[0b100_00000] = Self::arm_stm_ldm::<0b00000>;
        lut[0b100_00001] = Self::arm_stm_ldm::<0b00001>;
        lut[0b100_00010] = Self::arm_stm_ldm::<0b00010>;
        lut[0b100_00011] = Self::arm_stm_ldm::<0b00011>;
        lut[0b100_00100] = Self::arm_stm_ldm::<0b00100>;
        lut[0b100_00101] = Self::arm_stm_ldm::<0b00101>;
        lut[0b100_00110] = Self::arm_stm_ldm::<0b00110>;
        lut[0b100_00111] = Self::arm_stm_ldm::<0b00111>;
        lut[0b100_01000] = Self::arm_stm_ldm::<0b01000>;
        lut[0b100_01001] = Self::arm_stm_ldm::<0b01001>;
        lut[0b100_01010] = Self::arm_stm_ldm::<0b01010>;
        lut[0b100_01011] = Self::arm_stm_ldm::<0b01011>;
        lut[0b100_01100] = Self::arm_stm_ldm::<0b01100>;
        lut[0b100_01101] = Self::arm_stm_ldm::<0b01101>;
        lut[0b100_01110] = Self::arm_stm_ldm::<0b01110>;
        lut[0b100_01111] = Self::arm_stm_ldm::<0b01111>;
        lut[0b100_10000] = Self::arm_stm_ldm::<0b10000>;
        lut[0b100_10001] = Self::arm_stm_ldm::<0b10001>;
        lut[0b100_10010] = Self::arm_stm_ldm::<0b10010>;
        lut[0b100_10011] = Self::arm_stm_ldm::<0b10011>;
        lut[0b100_10100] = Self::arm_stm_ldm::<0b10100>;
        lut[0b100_10101] = Self::arm_stm_ldm::<0b10101>;
        lut[0b100_10110] = Self::arm_stm_ldm::<0b10110>;
        lut[0b100_10111] = Self::arm_stm_ldm::<0b10111>;
        lut[0b100_11000] = Self::arm_stm_ldm::<0b11000>;
        lut[0b100_11001] = Self::arm_stm_ldm::<0b11001>;
        lut[0b100_11010] = Self::arm_stm_ldm::<0b11010>;
        lut[0b100_11011] = Self::arm_stm_ldm::<0b11011>;
        lut[0b100_11100] = Self::arm_stm_ldm::<0b11100>;
        lut[0b100_11101] = Self::arm_stm_ldm::<0b11101>;
        lut[0b100_11110] = Self::arm_stm_ldm::<0b11110>;
        lut[0b100_11111] = Self::arm_stm_ldm::<0b11111>;

        lut[0b010_00000] = Self::arm_ldrstr::<0b00000, true>;
        lut[0b010_00001] = Self::arm_ldrstr::<0b00001, true>;
        lut[0b010_00010] = Self::arm_ldrstr::<0b00010, true>;
        lut[0b010_00011] = Self::arm_ldrstr::<0b00011, true>;
        lut[0b010_00100] = Self::arm_ldrstr::<0b00100, true>;
        lut[0b010_00101] = Self::arm_ldrstr::<0b00101, true>;
        lut[0b010_00110] = Self::arm_ldrstr::<0b00110, true>;
        lut[0b010_00111] = Self::arm_ldrstr::<0b00111, true>;
        lut[0b010_01000] = Self::arm_ldrstr::<0b01000, true>;
        lut[0b010_01001] = Self::arm_ldrstr::<0b01001, true>;
        lut[0b010_01010] = Self::arm_ldrstr::<0b01010, true>;
        lut[0b010_01011] = Self::arm_ldrstr::<0b01011, true>;
        lut[0b010_01100] = Self::arm_ldrstr::<0b01100, true>;
        lut[0b010_01101] = Self::arm_ldrstr::<0b01101, true>;
        lut[0b010_01110] = Self::arm_ldrstr::<0b01110, true>;
        lut[0b010_01111] = Self::arm_ldrstr::<0b01111, true>;
        lut[0b010_10000] = Self::arm_ldrstr::<0b10000, true>;
        lut[0b010_10001] = Self::arm_ldrstr::<0b10001, true>;
        lut[0b010_10010] = Self::arm_ldrstr::<0b10010, true>;
        lut[0b010_10011] = Self::arm_ldrstr::<0b10011, true>;
        lut[0b010_10100] = Self::arm_ldrstr::<0b10100, true>;
        lut[0b010_10101] = Self::arm_ldrstr::<0b10101, true>;
        lut[0b010_10110] = Self::arm_ldrstr::<0b10110, true>;
        lut[0b010_10111] = Self::arm_ldrstr::<0b10111, true>;
        lut[0b010_11000] = Self::arm_ldrstr::<0b11000, true>;
        lut[0b010_11001] = Self::arm_ldrstr::<0b11001, true>;
        lut[0b010_11010] = Self::arm_ldrstr::<0b11010, true>;
        lut[0b010_11011] = Self::arm_ldrstr::<0b11011, true>;
        lut[0b010_11100] = Self::arm_ldrstr::<0b11100, true>;
        lut[0b010_11101] = Self::arm_ldrstr::<0b11101, true>;
        lut[0b010_11110] = Self::arm_ldrstr::<0b11110, true>;
        lut[0b010_11111] = Self::arm_ldrstr::<0b11111, true>;
        lut[0b011_00000] = Self::arm_ldrstr::<0b00000, false>;
        lut[0b011_00001] = Self::arm_ldrstr::<0b00001, false>;
        lut[0b011_00010] = Self::arm_ldrstr::<0b00010, false>;
        lut[0b011_00011] = Self::arm_ldrstr::<0b00011, false>;
        lut[0b011_00100] = Self::arm_ldrstr::<0b00100, false>;
        lut[0b011_00101] = Self::arm_ldrstr::<0b00101, false>;
        lut[0b011_00110] = Self::arm_ldrstr::<0b00110, false>;
        lut[0b011_00111] = Self::arm_ldrstr::<0b00111, false>;
        lut[0b011_01000] = Self::arm_ldrstr::<0b01000, false>;
        lut[0b011_01001] = Self::arm_ldrstr::<0b01001, false>;
        lut[0b011_01010] = Self::arm_ldrstr::<0b01010, false>;
        lut[0b011_01011] = Self::arm_ldrstr::<0b01011, false>;
        lut[0b011_01100] = Self::arm_ldrstr::<0b01100, false>;
        lut[0b011_01101] = Self::arm_ldrstr::<0b01101, false>;
        lut[0b011_01110] = Self::arm_ldrstr::<0b01110, false>;
        lut[0b011_01111] = Self::arm_ldrstr::<0b01111, false>;
        lut[0b011_10000] = Self::arm_ldrstr::<0b10000, false>;
        lut[0b011_10001] = Self::arm_ldrstr::<0b10001, false>;
        lut[0b011_10010] = Self::arm_ldrstr::<0b10010, false>;
        lut[0b011_10011] = Self::arm_ldrstr::<0b10011, false>;
        lut[0b011_10100] = Self::arm_ldrstr::<0b10100, false>;
        lut[0b011_10101] = Self::arm_ldrstr::<0b10101, false>;
        lut[0b011_10110] = Self::arm_ldrstr::<0b10110, false>;
        lut[0b011_10111] = Self::arm_ldrstr::<0b10111, false>;
        lut[0b011_11000] = Self::arm_ldrstr::<0b11000, false>;
        lut[0b011_11001] = Self::arm_ldrstr::<0b11001, false>;
        lut[0b011_11010] = Self::arm_ldrstr::<0b11010, false>;
        lut[0b011_11011] = Self::arm_ldrstr::<0b11011, false>;
        lut[0b011_11100] = Self::arm_ldrstr::<0b11100, false>;
        lut[0b011_11101] = Self::arm_ldrstr::<0b11101, false>;
        lut[0b011_11110] = Self::arm_ldrstr::<0b11110, false>;
        lut[0b011_11111] = Self::arm_ldrstr::<0b11111, false>;

        if S::IS_V5 {
            Self::lut_span(&mut lut, 0b1110, 4, Self::armv5_cp15_trans);
        }

        lut
    }
}
