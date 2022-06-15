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
                    .unwrap_or(0)
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
            let res = Self::ror_s0(value, by);
            if by == 0 {
                self.set_zn(res);
            } else {
                self.set_znc(res, value.wshr(by.saturating_sub(1)).is_bit(0));
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
        let (_, carry) = rs.overflowing_add(rn);
        self.set_zncv::<false>(rs, rn, res, carry);
        res
    }

    /// Subtraction
    pub fn sub(&mut self, rs: u32, rn: u32) -> u32 {
        let res = rs.wrapping_sub(rn);
        let (_, carry) = rs.overflowing_sub(rn);
        self.set_zncv::<true>(rs, rn, res, !carry);
        res
    }

    /// Addition (c -> Carry)
    pub fn adc(&mut self, rs: u32, rn: u32, c: u32) -> u32 {
        let ab = self.add(rs, rn);
        let res = ab.wrapping_add(c);
        let (_, carry) = ab.overflowing_add(c);
        self.set_zn(res);
        self.set_flag(Flag::Carry, self.flag(Flag::Carry) | carry);
        self.set_flag(
            Flag::Overflow,
            self.flag(Flag::Overflow) | Self::is_v::<false>(ab, c, res),
        );
        res
    }

    /// Subtraction (c -> Carry)
    pub fn sbc(&mut self, rs: u32, rn: u32, c: u32) -> u32 {
        let ab = self.sub(rs, rn);
        let res = ab.wrapping_sub(c);
        let (_, carry) = ab.overflowing_sub(c);
        self.set_zn(res);
        self.set_flag(Flag::Carry, !self.flag(Flag::Carry) | !carry);
        self.set_flag(
            Flag::Overflow,
            self.flag(Flag::Overflow) | Self::is_v::<true>(ab, c, res),
        );
        res
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

    fn set_zncv<const SUB: bool>(&mut self, a: u32, b: u32, res: u32, carry: bool) {
        self.set_znc(res, carry);
        self.set_flag(Flag::Overflow, Self::is_v::<SUB>(a, b, res));
    }

    fn is_v<const SUB: bool>(a: u32, b: u32, res: u32) -> bool {
        let s1 = (a >> 31) != 0;
        let s2 = (b >> 31) != 0;
        let s3 = (res >> 31) != 0;
        if SUB {
            (!s1 && s2 && s3) || (s1 && !s2 && !s3)
        } else {
            (s1 && s2 && !s3) || (!s1 && !s2 && s3)
        }
    }
}
