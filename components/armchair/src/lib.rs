// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![no_std]

extern crate alloc;

mod arm;
mod exceptions;
pub mod interface;
mod memory;
mod misc;
pub mod optimizations;
pub mod state;
mod thumb;
pub mod tracing;

use alloc::vec::Vec;
use core::mem;

use arm::ArmInst;
use common::{numutil::NumExt, Time};
pub use exceptions::*;
use interface::Bus;
use memory::access::SEQ;
pub use memory::{access, Access, Address, RelativeOffset};
use misc::InstructionKind;
use optimizations::{
    cache::{CacheEntryKind, CacheStatus, CachedInstruction},
    CacheIndex, Optimizations,
};
pub use state::CpuState;
use thumb::ThumbInst;

use crate::{interface::RwType, state::Flag::Thumb};

/// Represents the CPU of the console.
/// It is generic over the system used; see `interface.rs`.
#[repr(C)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cpu<S: Bus> {
    pub state: CpuState,
    pub bus: S,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    pub opt: Optimizations<S>,
}

impl<S: Bus> Cpu<S> {
    /// Advance emulation.
    #[inline]
    pub fn continue_running(&mut self) {
        let pc = self.state.pc();
        if !self.bus.debugger().should_execute(pc.0) {
            return;
        }

        match mem::replace(&mut self.opt.cache, CacheStatus::JustInterpret) {
            CacheStatus::JustInterpret => self.interpret_next_instruction(),
            CacheStatus::MakeCacheNow => self.try_make_cache(None),
            CacheStatus::RunCacheNowAt(index) => {
                let block = self.opt.table.get_cache(index);
                self.run_cache_block(index, block);
            }
        }
    }

    /// Interpret the next instruction and advance the scheduler.
    fn interpret_next_instruction(&mut self) {
        self.bus.handle_events(&mut self.state);
        self.state.revalidate_pipeline(&mut self.bus);
        if self.state.is_flag(Thumb) {
            let (inst, _, _) = self.fetch_next_inst::<u16>();
            self.interpret_thumb(inst.u16());
        } else {
            let (inst, _, _) = self.fetch_next_inst::<u32>();
            self.interpret_arm(inst);
        }
    }

    /// Fetch the next instruction of the CPU.
    fn fetch_next_inst<TY: RwType>(&mut self) -> (u32, u16, Address) {
        let pc = self.state.bump_pc(TY::WIDTH);
        let access = self.state.access_type | access::CODE;
        let sn_cycles = self.bus.wait_time::<TY>(&mut self.state, pc, access);
        self.bus.tick(sn_cycles as u64);

        let future_inst = self.bus.get::<TY>(&mut self.state, pc).u32();
        let inst = self.state.advance_pipeline(future_inst);

        self.trace_inst::<TY>(inst);
        (inst, sn_cycles, pc)
    }

    fn try_make_cache(&mut self, index: Option<CacheIndex>) {
        self.state.revalidate_pipeline(&mut self.bus);
        self.opt
            .table
            .insert_cache(index, CacheEntryKind::FailedRetry);
        if self.state.current_instruction_type() == InstructionKind::Thumb {
            let mut block = Vec::with_capacity(10);
            while self.state.pipeline_valid {
                self.bus.handle_events(&mut self.state);
                if !self.state.pipeline_valid {
                    // CPU got interrupted by system, discard the block
                    self.opt
                        .table
                        .insert_cache(index, CacheEntryKind::FailedRetry);
                    return;
                }

                let (inst, cycles, _) = self.fetch_next_inst::<u16>();
                let instruction = inst.u16();
                let handler = Self::get_interpreter_handler_thumb(instruction);

                handler(self, ThumbInst::of(instruction));
                block.push(CachedInstruction {
                    instruction,
                    handler,
                    cycles: cycles as Time,
                });
                if self.state.pc().on_page_boundary() {
                    // Block hit a page boundary, finish the block
                    break;
                }
            }
            self.opt
                .table
                .insert_cache(index, CacheEntryKind::Thumb(block));
        } else {
            let mut block = Vec::with_capacity(10);
            while self.state.pipeline_valid {
                self.bus.handle_events(&mut self.state);
                if !self.state.pipeline_valid {
                    // CPU got interrupted by system, discard the block
                    self.opt
                        .table
                        .insert_cache(index, CacheEntryKind::FailedRetry);
                    return;
                }

                let (instruction, cycles, _) = self.fetch_next_inst::<u32>();
                let handler = Self::get_interpreter_handler_arm(instruction);

                if self.check_arm_cond(instruction) {
                    handler(self, ArmInst::of(instruction));
                }
                block.push(CachedInstruction {
                    instruction,
                    handler,
                    cycles: cycles as Time,
                });
                if self.state.pc().on_page_boundary() {
                    // Block hit a page boundary, finish the block
                    break;
                }
            }
            self.opt
                .table
                .insert_cache(index, CacheEntryKind::Arm(block));
        }
    }

    fn run_cache_block(&mut self, index: CacheIndex, block: &CacheEntryKind<S>) {
        self.state.revalidate_pipeline(&mut self.bus);
        match (block, self.state.current_instruction_type()) {
            (CacheEntryKind::Arm(cache), InstructionKind::Arm) => {
                for inst in cache.iter() {
                    self.bus.handle_events(&mut self.state);
                    if !self.state.pipeline_valid {
                        return;
                    }

                    let _pc = self.state.bump_pc(4);
                    self.bus.tick(inst.cycles);
                    if self.check_arm_cond(inst.instruction) {
                        self.state.access_type = SEQ;
                        (inst.handler)(self, ArmInst::of(inst.instruction));
                    }
                }
            }

            (CacheEntryKind::Thumb(cache), InstructionKind::Thumb) => {
                for inst in cache.iter() {
                    self.bus.handle_events(&mut self.state);
                    if !self.state.pipeline_valid {
                        return;
                    }

                    let _pc = self.state.bump_pc(2);
                    self.bus.tick(inst.cycles);
                    self.state.access_type = SEQ;
                    (inst.handler)(self, ThumbInst::of(inst.instruction));
                }
            }

            (CacheEntryKind::FailedRetry, _) => {
                // We failed making a cache block here in the past.
                // Try again.
                self.try_make_cache(Some(index));
            }

            // Edge case: Somehow we got a cache entry that doesn't match the CPU state
            // Just advance regularly and ignore the cache
            // (The only game known to me that triggers this is "Hello Kitty - Happy Party Pals")
            _ => self.interpret_next_instruction(),
        }
    }

    pub fn new(bus: S) -> Self {
        Self {
            bus,
            state: Default::default(),
            opt: Default::default(),
        }
    }
}
