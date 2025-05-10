use core::mem;

use cranelift::prelude::*;
use types::*;

use super::InstructionTranslator;
use crate::{interface::Bus, Cpu};

macro_rules! cpu_field {
    ($path:expr, $typ:expr, $getter:ident, $setter:ident) => {
        pub fn $getter(&mut self) -> Value {
            self.load_at_offset($typ, mem::offset_of!(Cpu<S>, $path))
        }
        pub fn $setter(&mut self, value: Value) {
            self.store_at_offset(value, mem::offset_of!(Cpu<S>, $path))
        }
    };
}

macro_rules! cpu_register {
    ($index:expr, $getter:ident, $setter:ident) => {
        pub fn $getter(&mut self) -> Value {
            self.load_at_offset(
                types::I32,
                mem::offset_of!(Cpu<S>, state.registers) + $index * mem::size_of::<u32>(),
            )
        }
        pub fn $setter(&mut self, value: Value) {
            self.store_at_offset(
                value,
                mem::offset_of!(Cpu<S>, state.registers) + $index * mem::size_of::<u32>(),
            )
        }
    };
}

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    fn load_at_offset(&mut self, typ: Type, offset: usize) -> Value {
        self.builder
            .ins()
            .load(typ, MemFlags::new(), self.vals.sys, offset as i32)
    }

    fn store_at_offset(&mut self, value: Value, offset: usize) {
        self.builder
            .ins()
            .store(MemFlags::new(), value, self.vals.sys, offset as i32);
    }

    fn get_pointer(&mut self, offset: usize) -> Value {
        let offset_const = self.builder.ins().iconst(types::I64, offset as i64);
        self.builder.ins().iadd(self.vals.sys, offset_const)
    }

    cpu_field!(state.pipeline_valid, I8, get_valid, set_valid);
    cpu_field!(state.cpsr, I32, get_cpsr, set_cpsr);

    cpu_register!(15, get_pc, set_pc);
}
