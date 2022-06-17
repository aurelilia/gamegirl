use crate::{
    gga::cpu::{registers::Flag, Cpu},
    numutil::NumExt,
};

impl Cpu {
    /// Logical/Arithmetic shift left
    pub fn lsl(&mut self, value: u32, by: u32) -> u32 {
        if by == 0 {
            self.set_zn(value);
            value
        } else {
            let res = value.wshl(by);
            self.set_znc(res, value.wshr(32u32.wrapping_sub(by)).is_bit(0));
            res
        }
    }

    /// Logical shift right
    pub fn lsr<const COERCE: bool>(&mut self, value: u32, by: u32) -> u32 {
        if by == 0 && COERCE {
            let res = value.wshr(32);
            self.set_znc(res, value.wshr(31).is_bit(0));
            res
        } else if by == 0 {
            self.set_zn(value);
            value
        } else {
            let res = value.wshr(by);
            self.set_znc(res, value.wshr(by.saturating_sub(1)).is_bit(0));
            res
        }
    }

    /// Arithmetic shift right
    pub fn asr<const COERCE: bool>(&mut self, value: u32, by: u32) -> u32 {
        if by == 0 && COERCE {
            let res = (value as i32).checked_shr(32).unwrap_or(value as i32 >> 31) as u32;
            self.set_znc(res, (value as i32).checked_shr(31).unwrap_or(0) & 1 == 1);
            res
        } else if by == 0 {
            self.set_zn(value);
            value
        } else {
            let res = (value as i32).checked_shr(by).unwrap_or(value as i32 >> 31) as u32;
            self.set_znc(
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
    pub fn ror<const COERCE: bool>(&mut self, value: u32, by: u32) -> u32 {
        if by == 0 && COERCE {
            let res = Self::ror_s0(value, 1).set_bit(31, self.flag(Flag::Carry));
            self.set_znc(res, value.is_bit(0));
            res
        } else {
            let res = Self::ror_s0(value, by & 31);
            if by == 0 {
                self.set_zn(res);
            } else {
                self.set_znc(res, res.is_bit(31));
            }
            res
        }
    }

    /// Rotate right, without setting CPSR
    pub fn ror_s0(value: u32, by: u32) -> u32 {
        value.rotate_right(by)
    }

    /// Addition
    pub fn add(&mut self, rs: u32, rn: u32) -> u32 {
        let res = rs.wrapping_add(rn);
        self.set_znc(res, (rs as u64) + (rn as u64) > 0xFFFF_FFFF);
        self.set_flag(Flag::Overflow, (rs as i32).overflowing_add(rn as i32).1);
        res
    }

    /// Subtraction
    pub fn sub(&mut self, rs: u32, rn: u32) -> u32 {
        let res = rs.wrapping_sub(rn);
        self.set_znc(res, rn <= rs);
        self.set_flag(Flag::Overflow, (rs as i32).overflowing_sub(rn as i32).1);
        res
    }

    /// Addition (c -> Carry)
    pub fn adc(&mut self, rs: u32, rn: u32, c: u32) -> u32 {
        let res = (rs as u64) + (rn as u64) + (c as u64);
        self.set_zn(res as u32);
        self.set_flag(Flag::Carry, res > 0xFFFF_FFFF);
        self.set_flag(
            Flag::Overflow,
            (!(rs ^ rn) & (rn ^ (res as u32))).is_bit(31),
        );
        res as u32
    }

    /// Subtraction (c -> Carry)
    pub fn sbc(&mut self, rs: u32, rn: u32, c: u32) -> u32 {
        self.adc(rs, !rn, c)
    }

    /// Multiplication
    pub fn mul(&mut self, a: u32, b: u32) -> u32 {
        let res = a.wrapping_mul(b);
        self.set_znc(res, false);
        res
    }

    /// Logic AND
    pub fn and(&mut self, a: u32, b: u32) -> u32 {
        let res = a & b;
        self.set_zn(res);
        res
    }

    /// Logic OR
    pub fn or(&mut self, a: u32, b: u32) -> u32 {
        let res = a | b;
        self.set_zn(res);
        res
    }

    /// Logic XOR
    pub fn xor(&mut self, a: u32, b: u32) -> u32 {
        let res = a ^ b;
        self.set_zn(res);
        res
    }

    /// Bit clear
    pub fn bit_clear(&mut self, a: u32, b: u32) -> u32 {
        let b = self.not(b);
        self.and(a, b)
    }

    pub fn not(&mut self, value: u32) -> u32 {
        let val = value ^ u32::MAX;
        self.set_zn(val);
        val
    }

    /// Negate
    pub fn neg(&mut self, value: u32) -> u32 {
        self.sub(0, value)
    }

    pub fn set_zn(&mut self, value: u32) {
        self.set_flag(Flag::Zero, value == 0);
        self.set_flag(Flag::Neg, value.is_bit(31));
    }

    fn set_znc(&mut self, value: u32, carry: bool) {
        self.set_zn(value);
        self.set_flag(Flag::Carry, carry);
    }
}
