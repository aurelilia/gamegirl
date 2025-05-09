use alloc::{format, vec::Vec};
use core::{marker::PhantomData, mem};

use common::numutil::NumExt;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use hashbrown::HashMap;

use super::analyze::BlockAnalysis;
use crate::{
    arm::{self, ArmInst},
    interface::Bus,
    misc::InstructionKind,
    thumb::{self, ThumbInst},
    Cpu, CpuState,
};

#[derive(Copy, Clone)]
pub struct JitBlock(usize);

impl JitBlock {
    pub fn call<S: Bus>(&self, cpu: &mut Cpu<S>) {
        unsafe {
            let inner: unsafe extern "C" fn(&mut Cpu<S>) = mem::transmute(self.0);
            (inner)(cpu);
        }
    }
}

pub struct Symbol {
    id: FuncId,
    kind: SymbolKind,
}

pub enum SymbolKind {
    JustCpu,
}

pub struct Jit<S: Bus> {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    data_description: DataDescription,
    module: JITModule,
    symbols: HashMap<&'static str, Symbol>,
    _s: PhantomData<S>,
}

impl<S: Bus> Default for Jit<S> {
    fn default() -> Self {
        // Functions on Cpu<S>
        let cpu_funcs: &[(&'static str, fn(&mut Cpu<S>))] =
            &[("cpu_print_thing", Cpu::print_thing)];

        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        for entry in cpu_funcs {
            builder.symbol(entry.0, entry.1 as *const _);
        }

        let mut module = JITModule::new(builder);
        let mut symbols = HashMap::new();

        {
            let ptr_ty = module.target_config().pointer_type();
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(ptr_ty));

            for func in cpu_funcs {
                let id = module
                    .declare_function(func.0, Linkage::Import, &sig)
                    .unwrap();
                symbols.insert(
                    func.0,
                    Symbol {
                        id,
                        kind: SymbolKind::JustCpu,
                    },
                );
            }
        }

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_description: DataDescription::new(),
            module,
            symbols,
            _s: PhantomData::default(),
        }
    }
}

impl<S: Bus> Jit<S> {
    pub fn compile(
        &mut self,
        index: usize,
        cpu: &mut CpuState,
        bus: &mut S,
        analysis: &BlockAnalysis,
    ) -> JitBlock {
        let ptr_ty = self.module.target_config().pointer_type();
        self.ctx.func.signature.params.push(AbiParam::new(ptr_ty)); // Parameter 1: System itself

        // Set up entry block
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        // Build the translator
        let sys = builder.block_params(entry)[0];
        let mut trans = InstructionTranslator {
            ana: analysis,
            sys,
            builder,
            module: &mut self.module,
            symbols: &self.symbols,

            cpu,
            bus,
        };

        // Translate instructions...
        trans.insert_preamble();
        // match analysis.kind {
        //     InstructionKind::Arm => {
        //         for instr in &analysis.instructions {
        //             let inst = ArmInst::of(instr.instr);
        //             let handle = arm::decode::get_instruction_handler(inst, false);
        //             handle(&mut trans, inst);
        //         }
        //     }
        //     InstructionKind::Thumb => {
        //         for instr in &analysis.instructions {
        //             let inst = ThumbInst::of(instr.instr.u16());
        //             let handle = thumb::decode::get_instruction_handler(inst);
        //             handle(&mut trans, inst);
        //         }
        //     }
        // }
        trans.insert_destructor();

        // Finalize the function and declare + define it
        trans.builder.ins().return_(&[]);
        trans.builder.finalize();
        let id = self
            .module
            .declare_function(
                &format!("jit{index}-{}", analysis.entry),
                Linkage::Export,
                &self.ctx.func.signature,
            )
            .unwrap();
        self.module.define_function(id, &mut self.ctx).unwrap();

        // Reset JIT state and finalize
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().unwrap();
        JitBlock(self.module.get_finalized_function(id) as usize)
    }
}

pub struct InstructionTranslator<'a, 'b, 'c, S: Bus> {
    ana: &'a BlockAnalysis,
    sys: Value,
    builder: FunctionBuilder<'b>,
    module: &'b mut JITModule,
    symbols: &'b HashMap<&'static str, Symbol>,

    cpu: &'c mut CpuState,
    bus: &'c mut S,
}

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    fn insert_preamble(&mut self) {
        self.call_symbol("cpu_print_thing");
    }

    fn insert_destructor(&mut self) {}

    fn call_symbol(&mut self, name: &'static str) {
        let symbol = self.symbols.get(name).unwrap();
        let local_callee = self
            .module
            .declare_func_in_func(symbol.id, &mut self.builder.func);

        let mut args = Vec::new();
        match symbol.kind {
            SymbolKind::JustCpu => {
                args.push(self.sys);
            }
        }

        self.builder.ins().call(local_callee, &args);
    }
}
