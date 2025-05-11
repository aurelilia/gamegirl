use common::numutil::NumExt;
use cranelift::prelude::*;

use super::{Condition, InstructionTranslator};
use crate::{interface::Bus, Cpu};

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn lsl(&mut self, set_cpsr: bool, value: Value, by: Value) -> Value {
        let res = self.builder.ins().ishl(value, by);
        if set_cpsr {
            let carry = {
                let shift = self.builder.ins().irsub_imm(by, 32);
                let shr = self.builder.ins().ushr(value, shift);
                let carry = self.builder.ins().band_imm(shr, 1);
                self.builder.ins().ireduce(types::I8, carry)
            };
            self.set_nzc(res, carry);
        }
        res
    }

    pub fn lsl_imm(&mut self, set_cpsr: bool, value: Value, by: u32) -> Value {
        if by == 0 {
            self.set_nz(value);
            value
        } else {
            let res = self.builder.ins().ishl_imm(value, by as i64);
            if set_cpsr {
                let carry = {
                    let shr = self
                        .builder
                        .ins()
                        .ushr_imm(value, 32u32.wrapping_sub(by) as i64);
                    let carry = self.builder.ins().band_imm(shr, 1);
                    self.builder.ins().ireduce(types::I8, carry)
                };
                self.set_nzc(res, carry);
            }
            res
        }
    }

    pub fn add(&mut self, set_cpsr: bool, a: Value, b: Value) -> Value {
        if set_cpsr {
            let (res, carry) = self.builder.ins().uadd_overflow(a, b);
            let (_, overflow) = self.builder.ins().sadd_overflow(a, b);
            self.set_nzcv(res, carry, overflow);
            res
        } else {
            self.builder.ins().iadd(a, b)
        }
    }

    pub fn add_imm(&mut self, set_cpsr: bool, value: Value, imm: u32) -> Value {
        if set_cpsr {
            let imm = self.builder.ins().iconst(types::I32, imm as i64);
            self.add(true, value, imm)
        } else {
            self.builder.ins().iadd_imm(value, imm as i64)
        }
    }

    pub fn sub(&mut self, set_cpsr: bool, a: Value, b: Value) -> Value {
        if set_cpsr {
            let (res, overflow) = self.builder.ins().ssub_overflow(a, b);
            let carry = self
                .builder
                .ins()
                .icmp(IntCC::UnsignedLessThanOrEqual, b, a);
            self.set_nzcv(res, carry, overflow);
            res
        } else {
            self.builder.ins().isub(a, b)
        }
    }

    pub fn sub_imm(&mut self, set_cpsr: bool, value: Value, imm: u32) -> Value {
        let imm = self.builder.ins().iconst(types::I32, imm as i64);
        self.sub(set_cpsr, value, imm)
    }

    pub fn mul(&mut self, set_cpsr: bool, a: Value, b: Value) -> Value {
        let res = self.builder.ins().imul(a, b);
        if set_cpsr {
            self.set_nzc(res, self.consts.false_i8);
        }
        res
    }

    pub fn and(&mut self, set_cpsr: bool, a: Value, b: Value) -> Value {
        let res = self.builder.ins().band(a, b);
        if set_cpsr {
            self.set_nz(res);
        }
        res
    }

    pub fn or(&mut self, set_cpsr: bool, a: Value, b: Value) -> Value {
        let res = self.builder.ins().bor(a, b);
        if set_cpsr {
            self.set_nz(res);
        }
        res
    }

    pub fn xor(&mut self, set_cpsr: bool, a: Value, b: Value) -> Value {
        let res = self.builder.ins().bxor(a, b);
        if set_cpsr {
            self.set_nz(res);
        }
        res
    }

    pub fn bit_clear(&mut self, set_cpsr: bool, a: Value, b: Value) -> Value {
        let res = self.builder.ins().band_not(a, b);
        if set_cpsr {
            self.set_nz(res);
        }
        res
    }

    pub fn not(&mut self, set_cpsr: bool, a: Value) -> Value {
        let res = self.builder.ins().bnot(a);
        if set_cpsr {
            self.set_nz(res);
        }
        res
    }

    pub fn neg(&mut self, set_cpsr: bool, a: Value) -> Value {
        self.sub(set_cpsr, self.consts.zero_i32, a)
    }

    pub fn set_nz(&mut self, value: Value) {
        self.call_set_nz(value);
    }

    pub fn set_nzc(&mut self, value: Value, carry: Value) {
        self.call_set_nzc(value, carry);
    }

    pub fn set_nzcv(&mut self, value: Value, carry: Value, overflow: Value) {
        self.call_set_nzcv(value, carry, overflow);
    }
}
