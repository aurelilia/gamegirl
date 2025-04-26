// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::NumExt;

use crate::{interface::Bus, state::Flag, Cpu};

impl<S: Bus> Cpu<S> {
    /// Logical/Arithmetic shift left
    pub fn lsl<const SET_CPSR: bool>(&mut self, value: u32, by: u32) -> u32 {
        if by == 0 {
            self.set_nz::<SET_CPSR>(value);
            value
        } else {
            let res = value.wshl(by);
            self.set_nzc::<SET_CPSR>(res, value.wshr(32u32.wrapping_sub(by)).is_bit(0));
            res
        }
    }

    /// Logical shift right
    pub fn lsr<const SET_CPSR: bool, const ZERO_IS_32: bool>(
        &mut self,
        value: u32,
        by: u32,
    ) -> u32 {
        if by == 0 && ZERO_IS_32 {
            let res = value.wshr(32);
            self.set_nzc::<SET_CPSR>(res, value.wshr(31).is_bit(0));
            res
        } else if by == 0 {
            self.set_nz::<SET_CPSR>(value);
            value
        } else {
            let res = value.wshr(by);
            self.set_nzc::<SET_CPSR>(res, value.wshr(by.saturating_sub(1)).is_bit(0));
            res
        }
    }

    /// Arithmetic shift right
    pub fn asr<const SET_CPSR: bool, const ZERO_IS_32: bool>(
        &mut self,
        value: u32,
        by: u32,
    ) -> u32 {
        if by == 0 && ZERO_IS_32 {
            let res = (value as i32).checked_shr(32).unwrap_or(value as i32 >> 31) as u32;
            self.set_nzc::<SET_CPSR>(res, (value as i32).checked_shr(31).unwrap_or(0) & 1 == 1);
            res
        } else if by == 0 {
            self.set_nz::<SET_CPSR>(value);
            value
        } else {
            let res = (value as i32).checked_shr(by).unwrap_or(value as i32 >> 31) as u32;
            self.set_nzc::<SET_CPSR>(
                res,
                (value as i32)
                    .checked_shr(by.saturating_sub(1))
                    .unwrap_or(value as i32 >> 31)
                    & 1
                    == 1,
            );
            res
        }
    }

    /// Rotate right
    pub fn ror<const SET_CPSR: bool, const ZERO_IS_1: bool>(&mut self, value: u32, by: u32) -> u32 {
        if by == 0 && ZERO_IS_1 {
            let res = value
                .rotate_right(1)
                .set_bit(31, self.state.is_flag(Flag::Carry));
            self.set_nzc::<SET_CPSR>(res, value.is_bit(0));
            res
        } else {
            let res = value.rotate_right(by & 31);
            if by == 0 {
                self.set_nz::<SET_CPSR>(res);
            } else {
                self.set_nzc::<SET_CPSR>(res, res.is_bit(31));
            }
            res
        }
    }

    /// Addition
    pub fn add<const SET_CPSR: bool>(&mut self, rs: u32, rn: u32) -> u32 {
        let res = rs.wrapping_add(rn);
        self.set_nzc::<SET_CPSR>(res, (rs as u64) + (rn as u64) > 0xFFFF_FFFF);
        self.set_flag_cpsr::<SET_CPSR>(Flag::Overflow, (rs as i32).overflowing_add(rn as i32).1);
        res
    }

    /// Subtraction
    pub fn sub<const SET_CPSR: bool>(&mut self, rs: u32, rn: u32) -> u32 {
        let res = rs.wrapping_sub(rn);
        self.set_nzc::<SET_CPSR>(res, rn <= rs);
        self.set_flag_cpsr::<SET_CPSR>(Flag::Overflow, (rs as i32).overflowing_sub(rn as i32).1);
        res
    }

    /// Addition (c -> Carry)
    pub fn adc<const SET_CPSR: bool>(&mut self, rs: u32, rn: u32, c: u32) -> u32 {
        let res = (rs as u64) + (rn as u64) + (c as u64);
        self.set_nz::<SET_CPSR>(res as u32);
        self.set_flag_cpsr::<SET_CPSR>(Flag::Carry, res > 0xFFFF_FFFF);
        self.set_flag_cpsr::<SET_CPSR>(
            Flag::Overflow,
            (!(rs ^ rn) & (rn ^ (res as u32))).is_bit(31),
        );
        res as u32
    }

    /// Subtraction (c -> Carry)
    pub fn sbc<const SET_CPSR: bool>(&mut self, rs: u32, rn: u32, c: u32) -> u32 {
        self.adc::<SET_CPSR>(rs, !rn, c)
    }

    /// Multiplication
    pub fn mul<const SET_CPSR: bool>(&mut self, a: u32, b: u32) -> u32 {
        let res = a.wrapping_mul(b);
        self.set_nzc::<SET_CPSR>(res, false);
        res
    }

    /// Logic AND
    pub fn and<const SET_CPSR: bool>(&mut self, a: u32, b: u32) -> u32 {
        let res = a & b;
        self.set_nz::<SET_CPSR>(res);
        res
    }

    /// Logic OR
    pub fn or<const SET_CPSR: bool>(&mut self, a: u32, b: u32) -> u32 {
        let res = a | b;
        self.set_nz::<SET_CPSR>(res);
        res
    }

    /// Logic XOR
    pub fn xor<const SET_CPSR: bool>(&mut self, a: u32, b: u32) -> u32 {
        let res = a ^ b;
        self.set_nz::<SET_CPSR>(res);
        res
    }

    /// Bit clear
    pub fn bit_clear<const SET_CPSR: bool>(&mut self, a: u32, b: u32) -> u32 {
        let b = self.not::<SET_CPSR>(b);
        self.and::<SET_CPSR>(a, b)
    }

    /// Not
    pub fn not<const SET_CPSR: bool>(&mut self, value: u32) -> u32 {
        let val = value ^ u32::MAX;
        self.set_nz::<SET_CPSR>(val);
        val
    }

    /// Negate
    pub fn neg<const SET_CPSR: bool>(&mut self, value: u32) -> u32 {
        self.sub::<SET_CPSR>(0, value)
    }

    pub fn set_nz<const ENABLE: bool>(&mut self, value: u32) {
        if ENABLE {
            let neg = value & (1 << 31);
            let zero = ((value == 0) as u32) << 30;
            self.state
                .set_cpsr_flags((self.state.cpsr() & 0x3FFF_FFFF) | zero | neg);
        }
    }

    fn set_nzc<const ENABLE: bool>(&mut self, value: u32, carry: bool) {
        if ENABLE {
            let neg = value & (1 << 31);
            let zero = ((value == 0) as u32) << 30;
            let carry = (carry as u32) << 29;
            self.state
                .set_cpsr_flags((self.state.cpsr() & 0x1FFF_FFFF) | zero | neg | carry);
        }
    }

    fn set_flag_cpsr<const ENABLE: bool>(&mut self, flag: Flag, en: bool) {
        if ENABLE {
            self.state
                .set_cpsr_flags(self.state.cpsr().set_bit(flag as u16, en));
        }
    }
}
