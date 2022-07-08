// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! This file contains a bunch of math operations that are out of scope for
//! the GameGirl module file.

use crate::{
    ggc::{
        cpu::{DReg, Flag, Reg},
        GameGirl,
    },
    numutil::NumExt,
};

impl GameGirl {
    // c is the value of the carry, only used by ADC
    pub(super) fn add(&mut self, a: u16, b: u16, c: u16, set_carry: bool) -> u16 {
        let result = a + b + c;
        self.cpu.set_fl(Flag::Zero, (result & 0xFF) == 0);
        self.cpu.set_fl(Flag::Negative, false);
        self.cpu
            .set_fli16(Flag::HalfCarry, ((a & 0xF) + (b & 0xF) + c) & 0x10);
        if set_carry {
            self.cpu
                .set_fli16(Flag::Carry, ((a & 0xFF) + (b & 0xFF) + c) & 0x100);
        }
        result & 0xFF
    }

    // c is the value of the carry, only used by SBC
    pub(super) fn sub(&mut self, a: u16, b: u16, c: u16, set_carry: bool) -> u16 {
        let result = a.wrapping_sub(b).wrapping_sub(c);
        self.cpu.set_fl(Flag::Zero, (result & 0xFF) == 0);
        self.cpu.set_fl(Flag::Negative, true);
        self.cpu.set_fli16(
            Flag::HalfCarry,
            ((a & 0xF).wrapping_sub(b & 0xF).wrapping_sub(c)) & 0x10,
        );
        if set_carry {
            self.cpu.set_fli16(
                Flag::Carry,
                ((a & 0xFF).wrapping_sub(b & 0xFF).wrapping_sub(c)) & 0x100,
            );
        }
        result & 0xFF
    }

    pub(super) fn add_16_hl(&mut self, other: u16) {
        let hl = self.cpu.dreg(DReg::HL);
        let result = hl.wrapping_add(other);
        self.cpu.set_fl(Flag::Negative, false);
        self.cpu
            .set_fli16(Flag::HalfCarry, ((hl & 0xFFF) + (other & 0xFFF)) & 0x1000);
        self.cpu
            .set_fl(Flag::Carry, ((hl as u32 + other as u32) & 0x10000) != 0);
        self.cpu.set_dreg(DReg::HL, result);
        self.advance_clock(1); // Internal delay
    }

    pub(super) fn mod_ret_hl(&mut self, mod_: i32) -> u16 {
        let ret = self.cpu.dreg(DReg::HL);
        self.cpu.set_dreg(DReg::HL, (ret as i32 + mod_) as u16);
        ret
    }

    // Thanks to https://stackoverflow.com/questions/5159603/gbz80-how-does-ld-hl-spe-affect-h-and-c-flags
    // as well as kotcrab's xgbc emulator for showing me correct behavior for 0xE8 &
    // 0xF8!
    pub(super) fn add_sp(&mut self) -> u16 {
        let value = self.read_s8(self.cpu.pc + 1) as i16;
        self.cpu.set_fl(Flag::Zero, false);
        self.cpu.set_fl(Flag::Negative, false);
        self.cpu.set_fli16(
            Flag::HalfCarry,
            ((self.cpu.sp & 0xF).wrapping_add_signed(value & 0xF)) & 0x10,
        );
        self.cpu.set_fli16(
            Flag::Carry,
            ((self.cpu.sp & 0xFF).wrapping_add_signed(value & 0xFF)) & 0x100,
        );
        self.cpu.sp.wrapping_add_signed(value)
    }

    pub(super) fn rlc(&mut self, value: u8, maybe_set_z: bool) -> u8 {
        let result = value.rotate_left(1);
        self.cpu.set_reg(
            Reg::F,
            Flag::Carry.from(value.bit(7))
                + if maybe_set_z && result == 0 {
                    Flag::Zero.mask().u8()
                } else {
                    0
                },
        );
        result
    }

    pub(super) fn rrc(&mut self, value: u8, maybe_set_z: bool) -> u8 {
        let result = value.rotate_right(1);
        self.cpu.set_reg(
            Reg::F,
            Flag::Carry.from(value.bit(0))
                + if maybe_set_z && result == 0 {
                    Flag::Zero.mask()
                } else {
                    0
                },
        );
        result
    }

    pub(super) fn rl(&mut self, value: u8, maybe_set_z: bool) -> u8 {
        let result = value.rotate_left(1).set_bit(0, self.cpu.flag(Flag::Carry));
        self.cpu.set_reg(
            Reg::F,
            Flag::Carry.from(value.bit(7))
                + if maybe_set_z && result == 0 {
                    Flag::Zero.mask()
                } else {
                    0
                },
        );
        result.u8()
    }

    pub(super) fn rr(&mut self, value: u8, maybe_set_z: bool) -> u8 {
        let result = value.rotate_right(1).set_bit(7, self.cpu.flag(Flag::Carry));
        self.cpu.set_reg(
            Reg::F,
            Flag::Carry.from(value.bit(0))
                + if maybe_set_z && result == 0 {
                    Flag::Zero.mask()
                } else {
                    0
                },
        );
        result.u8()
    }

    pub(super) fn sla(&mut self, value: u8) -> u8 {
        let result = (value.u16() << 1) & 0xFF;
        self.cpu.set_reg(
            Reg::F,
            Flag::Carry.from(value.bit(7)) + if result == 0 { Flag::Zero.mask() } else { 0 },
        );
        result.u8()
    }

    pub(super) fn sra(&mut self, value: u8) -> u8 {
        let a = value >> 1;
        let result = a.set_bit(7, value.is_bit(7));
        self.cpu.set_reg(
            Reg::F,
            Flag::Carry.from(value.bit(0)) + if result == 0 { Flag::Zero.mask() } else { 0 },
        );
        result.u8()
    }

    pub(super) fn swap(&mut self, value: u8) -> u8 {
        let upper = value.u16() >> 4;
        let lower = (value.u16() & 0xF) << 4;
        self.cpu.set_reg(
            Reg::F,
            if (upper + lower) == 0 {
                Flag::Zero.mask()
            } else {
                0
            },
        );
        (lower + upper).u8()
    }

    pub(super) fn srl(&mut self, value: u8) -> u8 {
        let result = (value.u16() >> 1) & 0xFF;
        self.cpu.set_reg(
            Reg::F,
            Flag::Carry.from(value.bit(0)) + if result == 0 { Flag::Zero.mask() } else { 0 },
        );
        result.u8()
    }

    pub(super) fn bit(&mut self, value: u8, bit: u16) -> u8 {
        self.cpu.set_fli(Flag::Zero, value.bit(bit) ^ 1);
        self.cpu.set_fl(Flag::Negative, false);
        self.cpu.set_fl(Flag::HalfCarry, true);
        value
    }

    pub(super) fn z_flag_only(&mut self, value: u8) -> u8 {
        if value == 0 {
            self.cpu.set_reg(Reg::F, Flag::Zero.mask())
        } else {
            self.cpu.set_reg(Reg::F, 0)
        }
        value
    }
}
