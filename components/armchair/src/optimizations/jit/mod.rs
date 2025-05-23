use alloc::{format, vec};
use core::mem;

use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, ModuleResult};
use ffi::{DefinedSymbolTable, SymbolTable, SYM_COUNT};
use hashbrown::HashMap;

use super::analyze::BlockAnalysis;
use crate::{interface::Bus, misc::InstructionKind, Address, Cpu, CpuState};

mod alu;
mod codegen;
mod ffi;
mod fields;
mod support;

#[derive(Copy, Clone)]
pub enum Condition {
    RunNever,
    RunAlways,
    RunIf(Value),
}

#[derive(Copy, Clone)]
pub struct JitBlock {
    inner: usize,
    entry: Address,
}

impl JitBlock {
    pub fn call<S: Bus>(&self, cpu: &mut Cpu<S>) {
        if self.entry != cpu.state.pc() {
            log::error!("THIS SHOULD NEVER HAPPEN: JIT block to be executed in wrong location!");
            return;
        }
        unsafe {
            let inner: unsafe extern "C" fn(&mut Cpu<S>) = mem::transmute(self.inner);
            (inner)(cpu);
        }
    }
}

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: cranelift::codegen::Context,
    module: JITModule,
    symbols: SymbolTable,
    targets_map: HashMap<Address, Block>,
    pub stats: JitStats,
}

impl Jit {
    pub fn compile<S: Bus>(
        &mut self,
        index: usize,
        cpu: &mut CpuState,
        bus: &mut S,
        analysis: &BlockAnalysis,
    ) -> JitBlock {
        self.targets_map.clear();
        let ptr_ty = self.module.target_config().pointer_type();
        self.ctx.func.signature.params.push(AbiParam::new(ptr_ty)); // Parameter 1: System itself

        // Set up entry block
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        // Set up return block
        let abort_block = builder.create_block();

        // Build the translator
        let sys = builder.block_params(entry)[0];
        let bus_offset = builder
            .ins()
            .iconst(ptr_ty, mem::offset_of!(Cpu<S>, bus) as i64);
        let bus_val = builder.ins().iadd(sys, bus_offset);
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let one_i32 = builder.ins().iconst(types::I32, 1);
        let two_i32 = builder.ins().iconst(types::I32, 2);
        let four_i32 = builder.ins().iconst(types::I32, 4);
        let mut trans = InstructionTranslator {
            ana: analysis,
            builder,
            module: &mut self.module,
            symbols: &self.symbols,
            defined_symbols: [None; SYM_COUNT],
            current_instruction: analysis.entry,
            instruction_target_blocks: &mut self.targets_map,
            instructions_since_sync: 0,
            wait_time_collected: 0,

            cpu,
            bus,

            vals: Values {
                sys,
                bus: bus_val,
                abort_block,
            },
            consts: Constants {
                zero_i32,
                one_i32,
                two_i32,
                four_i32,
            },
            stats: &mut self.stats,
        };

        // Translate instructions...
        trans.insert_function_preamble();
        match analysis.kind {
            InstructionKind::Arm => {
                for instr in &analysis.instructions {
                    trans.translate_arm(instr);
                    trans.current_instruction += Address::WORD;
                }
            }
            InstructionKind::Thumb => {
                for instr in &analysis.instructions {
                    trans.translate_thumb(instr);
                    trans.current_instruction += Address::HW;
                }
            }
        }
        trans.insert_function_exit();

        // Finalize the function and declare + define it
        trans.builder.ins().jump(abort_block, &[]);
        trans.builder.switch_to_block(abort_block);
        trans.builder.ins().return_(&[]);
        trans.builder.finalize();

        let result: ModuleResult<usize> = (|| {
            let id = self.module.declare_function(
                &format!("jit{index}-{}", analysis.entry),
                Linkage::Export,
                &self.ctx.func.signature,
            )?;
            self.module.define_function(id, &mut self.ctx)?;

            // Reset JIT state and finalize
            self.module.clear_context(&mut self.ctx);
            self.module.finalize_definitions()?;
            let inner = self.module.get_finalized_function(id) as usize;
            Ok(inner)
        })();

        match result {
            Ok(inner) => JitBlock {
                inner,
                entry: analysis.entry,
            },
            Err(err) => {
                panic!("{:#?} during function {}", err, self.ctx.func);
            }
        }
    }

    pub fn new<S: Bus>() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let (module, symbols) = ffi::get_module_with_symbols::<S>(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            symbols,
            targets_map: HashMap::with_capacity(5),
            stats: JitStats::default(),
        }
    }
}

pub struct InstructionTranslator<'a, 'b, 'c, S: Bus> {
    pub ana: &'a BlockAnalysis,
    pub builder: FunctionBuilder<'b>,
    module: &'b mut JITModule,
    symbols: &'b SymbolTable,
    defined_symbols: DefinedSymbolTable,

    pub current_instruction: Address,
    instruction_target_blocks: &'b mut HashMap<Address, Block>,

    instructions_since_sync: usize,
    wait_time_collected: usize,

    pub cpu: &'c mut CpuState,
    pub bus: &'c mut S,
    pub vals: Values,
    pub consts: Constants,
    pub stats: &'b mut JitStats,
}

pub struct Values {
    pub sys: Value,
    pub bus: Value,
    pub abort_block: Block,
}

pub struct Constants {
    pub zero_i32: Value,
    pub one_i32: Value,
    pub two_i32: Value,
    pub four_i32: Value,
}

#[derive(Debug, Default)]
pub struct JitStats {
    pub interpreted_instructions: usize,
    pub fallback_instructions: usize,
    pub native_instructions: usize,

    pub thumb_instructions: usize,
    pub arm_instructions: usize,
}

impl JitStats {
    pub fn total(&self) -> usize {
        self.interpreted_instructions + self.fallback_instructions + self.native_instructions
    }

    pub fn arm_percent(&self) -> f64 {
        self.arm_instructions as f64 / (self.thumb_instructions + self.arm_instructions) as f64
    }

    pub fn thumb_percent(&self) -> f64 {
        self.thumb_instructions as f64 / (self.thumb_instructions + self.arm_instructions) as f64
    }

    pub fn interpreted_percent(&self) -> f64 {
        self.interpreted_instructions as f64 / self.total() as f64
    }

    pub fn fallback_percent(&self) -> f64 {
        self.fallback_instructions as f64 / self.total() as f64
    }

    pub fn native_percent(&self) -> f64 {
        self.native_instructions as f64 / self.total() as f64
    }
}
