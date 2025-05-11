use alloc::vec::Vec;

use cranelift::{
    codegen::ir::{FuncRef, Inst},
    prelude::*,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use super::InstructionTranslator;
use crate::{interface::Bus, Cpu};

pub const SYM_COUNT: usize = 9;

pub type SymbolTable = Vec<FuncId>;
pub type DefinedSymbolTable = [Option<FuncRef>; SYM_COUNT];

pub fn get_module_with_symbols<S: Bus>(mut builder: JITBuilder) -> (JITModule, SymbolTable) {
    fn apply(sig: &mut Signature, params: &[Type], ret: &[Type]) {
        sig.params.clear();
        sig.params.extend(params.iter().copied().map(AbiParam::new));
        sig.returns.clear();
        sig.returns.extend(ret.iter().copied().map(AbiParam::new));
    }

    let syms = get_table::<S>();
    for (name, ptr, _, _) in syms {
        builder.symbol(*name, *ptr);
    }

    let mut module = JITModule::new(builder);
    let mut sig = module.make_signature();
    let symbols = syms
        .iter()
        .map(|(name, _, params, ret)| {
            apply(&mut sig, params, ret);
            module
                .declare_function(name, Linkage::Import, &sig)
                .unwrap()
        })
        .collect();

    (module, symbols)
}

fn get_table<S: Bus>(
) -> &'static [(&'static str, *const u8, &'static [Type], &'static [Type]); SYM_COUNT] {
    &[
        (
            "handle_events",
            S::handle_events as *const _,
            &[types::I64, types::I64],
            &[],
        ),
        ("tick", S::tick as *const _, &[types::I64, types::I64], &[]),
        (
            "interpret_thumb",
            Cpu::<S>::interpret_thumb as *const _,
            &[types::I64, types::I16],
            &[],
        ),
        (
            "interpret_arm",
            Cpu::<S>::interpret_arm as *const _,
            &[types::I64, types::I32],
            &[],
        ),
        (
            "trace_inst_thumb",
            Cpu::<S>::trace_inst::<u16> as *const _,
            &[types::I64, types::I32],
            &[],
        ),
        (
            "trace_inst_arm",
            Cpu::<S>::trace_inst::<u32> as *const _,
            &[types::I64, types::I32],
            &[],
        ),
        (
            "set_nz",
            Cpu::<S>::set_nz_ as *const _,
            &[types::I64, types::I32],
            &[],
        ),
        (
            "set_nzc",
            Cpu::<S>::set_nzc_ as *const _,
            &[types::I64, types::I32, types::I8],
            &[],
        ),
        (
            "set_nzcv",
            Cpu::<S>::set_nzcv_ as *const _,
            &[types::I64, types::I32, types::I8, types::I8],
            &[],
        ),
    ]
}

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn bus_handle_events(&mut self) {
        self.call_with(0, &[self.vals.bus, self.vals.sys]);
    }

    pub fn bus_tick(&mut self, time: Value) {
        self.call_with(1, &[self.vals.bus, time]);
    }

    pub fn interpret_thumb(&mut self, instr: Value) {
        self.call_with(2, &[self.vals.sys, instr]);
    }

    pub fn interpret_arm(&mut self, instr: Value) {
        self.call_with(3, &[self.vals.sys, instr]);
    }

    pub fn trace_inst_thumb(&mut self, instr: Value) {
        self.call_with(4, &[self.vals.sys, instr]);
    }

    pub fn trace_inst_arm(&mut self, instr: Value) {
        self.call_with(5, &[self.vals.sys, instr]);
    }

    pub fn set_nz(&mut self, value: Value) {
        self.call_with(6, &[self.vals.sys, value]);
    }

    pub fn set_nzc(&mut self, value: Value, c: Value) {
        self.call_with(7, &[self.vals.sys, value, c]);
    }

    pub fn set_nzcv(&mut self, value: Value, c: Value, v: Value) {
        self.call_with(8, &[self.vals.sys, value, c, v]);
    }

    fn call_with(&mut self, index: usize, args: &[Value]) -> Inst {
        let local_callee = self.local_callee(index);
        self.builder.ins().call(local_callee, args)
    }

    fn local_callee(&mut self, index: usize) -> FuncRef {
        if let Some(sym) = self.defined_symbols.get(index).copied().flatten() {
            sym
        } else {
            let symbol = self.get_symbol(index);
            let local_callee = self
                .module
                .declare_func_in_func(symbol, &mut self.builder.func);
            self.defined_symbols[index] = Some(local_callee);
            local_callee
        }
    }

    fn get_symbol(&mut self, index: usize) -> FuncId {
        *self.symbols.get(index).unwrap()
    }
}
