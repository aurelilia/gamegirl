use cranelift::{codegen::ir::FuncRef, prelude::*};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use hashbrown::HashMap;

use super::InstructionTranslator;
use crate::{interface::Bus, Cpu, CpuState};

#[derive(Debug, Copy, Clone)]
pub struct Symbol {
    id: FuncId,
    kind: SymbolKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Cpu,
    CpuWithI16,
    CpuWithI32,
    BusWithState,
    BusWithI64,
    Misc,
}

pub type SymbolTable = HashMap<&'static str, Symbol>;
pub type DefinedSymbolTable = HashMap<&'static str, FuncRef>;

pub type CpuOnlySymbol<S> = fn(&mut Cpu<S>);
pub type CpuWithU16Symbol<S> = fn(&mut Cpu<S>, u16);
pub type CpuWithU32Symbol<S> = fn(&mut Cpu<S>, u32);
pub type BusWithStateSymbol<S> = fn(&mut S, &mut CpuState);
pub type BusWithU64Symbol<S> = fn(&mut S, u64);

pub fn get_module_with_symbols<S: Bus>(mut builder: JITBuilder) -> (JITModule, SymbolTable) {
    let cpu_funcs: &[(&'static str, CpuOnlySymbol<S>)] =
        &[("cpu_print_thing", Cpu::setup_cpu_state)];
    let bus_with_state_funcs: &[(&'static str, BusWithStateSymbol<S>)] =
        &[("bus_handle_events", S::handle_events)];

    for entry in cpu_funcs {
        builder.symbol(entry.0, entry.1 as *const _);
    }
    for entry in bus_with_state_funcs {
        builder.symbol(entry.0, entry.1 as *const _);
    }
    builder.symbol("cpu_interpret_thumb", Cpu::<S>::interpret_thumb as *const _);
    builder.symbol("cpu_interpret_arm", Cpu::<S>::interpret_arm as *const _);
    builder.symbol("bus_tick", S::tick as *const _);
    builder.symbol("cpu_trace_thumb", Cpu::<S>::trace_inst::<u16> as *const _);
    builder.symbol("cpu_trace_arm", Cpu::<S>::trace_inst::<u32> as *const _);
    builder.symbol("set_nz", Cpu::<S>::set_nz_ as *const _);
    builder.symbol("set_nzc", Cpu::<S>::set_nzc_ as *const _);
    builder.symbol("set_nzcv", Cpu::<S>::set_nzcv_ as *const _);

    let mut module = JITModule::new(builder);
    let mut symbols = HashMap::new();

    // TABLES
    let ptr_ty = module.target_config().pointer_type();
    let mut sig = module.make_signature();
    {
        sig.params.push(AbiParam::new(ptr_ty));

        for func in cpu_funcs {
            let id = module
                .declare_function(func.0, Linkage::Import, &sig)
                .unwrap();
            symbols.insert(
                func.0,
                Symbol {
                    id,
                    kind: SymbolKind::Cpu,
                },
            );
        }
        sig.clear(module.isa().default_call_conv());
    }
    {
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(ptr_ty));

        for func in bus_with_state_funcs {
            let id = module
                .declare_function(func.0, Linkage::Import, &sig)
                .unwrap();
            symbols.insert(
                func.0,
                Symbol {
                    id,
                    kind: SymbolKind::BusWithState,
                },
            );
        }
        sig.clear(module.isa().default_call_conv());
    }

    // MISC
    {
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I16));
        let id = module
            .declare_function("cpu_interpret_thumb", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "cpu_interpret_thumb",
            Symbol {
                id,
                kind: SymbolKind::CpuWithI16,
            },
        );
        sig.clear(module.isa().default_call_conv());
    }
    {
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I64));
        let id = module
            .declare_function("bus_tick", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "bus_tick",
            Symbol {
                id,
                kind: SymbolKind::BusWithI64,
            },
        );
        sig.clear(module.isa().default_call_conv());
    }
    {
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        let id = module
            .declare_function("cpu_trace_thumb", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "cpu_trace_thumb",
            Symbol {
                id,
                kind: SymbolKind::CpuWithI32,
            },
        );
        let id = module
            .declare_function("cpu_trace_arm", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "cpu_trace_arm",
            Symbol {
                id,
                kind: SymbolKind::CpuWithI32,
            },
        );
        let id = module
            .declare_function("cpu_interpret_arm", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "cpu_interpret_arm",
            Symbol {
                id,
                kind: SymbolKind::CpuWithI32,
            },
        );
        sig.clear(module.isa().default_call_conv());
    }
    {
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        let id = module
            .declare_function("set_nz", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "set_nz",
            Symbol {
                id,
                kind: SymbolKind::Misc,
            },
        );
        sig.params.push(AbiParam::new(types::I8));
        let id = module
            .declare_function("set_nzc", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "set_nzc",
            Symbol {
                id,
                kind: SymbolKind::Misc,
            },
        );
        sig.params.push(AbiParam::new(types::I8));
        let id = module
            .declare_function("set_nzcv", Linkage::Import, &sig)
            .unwrap();
        symbols.insert(
            "set_nzcv",
            Symbol {
                id,
                kind: SymbolKind::Misc,
            },
        );

        sig.clear(module.isa().default_call_conv());
    }

    (module, symbols)
}

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn call_cpu(&mut self, fun: CpuOnlySymbol<S>) {
        let local_callee = self.local_callee("cpu_print_thing", SymbolKind::Cpu);
        self.builder.ins().call(local_callee, &[self.vals.sys]);
    }

    pub fn call_cpui16(&mut self, fun: CpuWithU16Symbol<S>, value: Value) {
        let local_callee = self.local_callee("cpu_interpret_thumb", SymbolKind::CpuWithI16);
        self.builder
            .ins()
            .call(local_callee, &[self.vals.sys, value]);
    }

    pub fn call_cpui32(&mut self, fun: CpuWithU32Symbol<S>, name: &'static str, value: Value) {
        let local_callee = self.local_callee(name, SymbolKind::CpuWithI32);
        self.builder
            .ins()
            .call(local_callee, &[self.vals.sys, value]);
    }

    pub fn call_set_nz(&mut self, a: Value) {
        let local_callee = self.local_callee("set_nz", SymbolKind::Misc);
        self.builder.ins().call(local_callee, &[self.vals.sys, a]);
    }

    pub fn call_set_nzc(&mut self, a: Value, c: Value) {
        let local_callee = self.local_callee("set_nzc", SymbolKind::Misc);
        self.builder
            .ins()
            .call(local_callee, &[self.vals.sys, a, c]);
    }

    pub fn call_set_nzcv(&mut self, a: Value, c: Value, o: Value) {
        let local_callee = self.local_callee("set_nzcv", SymbolKind::Misc);
        self.builder
            .ins()
            .call(local_callee, &[self.vals.sys, a, c, o]);
    }

    pub fn call_buscpu(&mut self, fun: BusWithStateSymbol<S>) {
        let local_callee = self.local_callee("bus_handle_events", SymbolKind::BusWithState);
        self.builder
            .ins()
            .call(local_callee, &[self.vals.bus, self.vals.sys]);
    }

    pub fn call_busi64(&mut self, fun: BusWithU64Symbol<S>, value: Value) {
        let local_callee = self.local_callee("bus_tick", SymbolKind::BusWithI64);
        self.builder
            .ins()
            .call(local_callee, &[self.vals.bus, value]);
    }

    fn local_callee(&mut self, name: &'static str, kind: SymbolKind) -> FuncRef {
        if let Some(sym) = self.defined_symbols.get(name) {
            *sym
        } else {
            let symbol = self.get_symbol(name);
            assert_eq!(symbol.kind, kind);
            let local_callee = self
                .module
                .declare_func_in_func(symbol.id, &mut self.builder.func);
            self.defined_symbols.insert(name, local_callee);
            local_callee
        }
    }

    fn get_symbol(&mut self, name: &'static str) -> Symbol {
        *self.symbols.get(name).unwrap()
    }
}
