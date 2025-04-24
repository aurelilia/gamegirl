// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![no_std]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

extern crate alloc;

mod arm;
mod exceptions;
pub mod interface;
mod memory;
mod misc;
mod optimizations;
pub mod registers;
mod thumb;
pub mod tracing;

use common::{common::debugger::Debugger, numutil::NumExt};
pub use exceptions::*;
use interface::Bus;
use memory::{
    access::{CODE, NONSEQ, SEQ},
    Access, Address,
};
use optimizations::{cache::Cache, waitloop::WaitloopData};
use registers::Registers;

use crate::{interface::RwType, registers::Flag::Thumb};

// TODO initial pipeline fill

/// Represents the CPU of the console - an ARM7TDMI.
/// It is generic over the system used; see `interface.rs`.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cpu<S: Bus> {
    pub bus: S,
    pub debugger: Debugger,

    pub regs: Registers,
    pub pipeline: [u32; 2],
    pub access_type: Access,
    pub is_halted: bool,

    pub ime: bool,
    pub ie: u32,
    pub if_: u32,

    block_ended: bool,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    pub cache: Cache<S>,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    waitloop: WaitloopData,
}

impl<S: Bus> Cpu<S> {
    /// Advance emulation.
    #[inline]
    pub fn continue_running(&mut self) {
        let pc = self.regs.pc();
        if !self.debugger.should_execute(pc.0) {
            return;
        }

        self.interpret_next_instruction();
    }

    /// Interpret the next instruction and advance the scheduler.
    fn interpret_next_instruction(&mut self) {
        self.bus.handle_events();
        if self.regs.is_flag(Thumb) {
            let (inst, _, _) = self.fetch_next_inst::<u16>();
            self.interpret_thumb(inst.u16());
        } else {
            let (inst, _, _) = self.fetch_next_inst::<u32>();
            self.interpret_arm(inst);
        }
    }

    /// Fetch the next instruction of the CPU.
    fn fetch_next_inst<TY: RwType>(&mut self) -> (u32, u16, Address) {
        let pc = self.regs.bump_pc(TY::WIDTH);
        let sn_cycles = self
            .bus
            .wait_time::<TY>(self.regs.pc(), self.access_type | CODE);
        self.bus.tick(sn_cycles as u64);

        let inst = self.pipeline[0];
        self.pipeline[0] = self.pipeline[1];
        self.pipeline[1] = self.bus.get::<TY>(self.regs.pc()).u32();
        self.access_type = SEQ;

        self.trace_inst::<TY>(inst);
        (inst, sn_cycles, pc)
    }

    /// Emulate a pipeline stall / fill; used when PC changes.
    pub fn pipeline_stall(&mut self) {
        self.bus.pipeline_stalled();
        if self.regs.is_flag(Thumb) {
            let time = self.bus.wait_time::<u16>(self.regs.pc(), NONSEQ | CODE);
            self.bus.tick(time as u64);
            self.pipeline[0] = self.bus.get::<u16>(self.regs.pc()).u32();

            self.regs.bump_pc(2);
            let time = self.bus.wait_time::<u16>(self.regs.pc(), SEQ | CODE);
            self.bus.tick(time as u64);
            self.pipeline[1] = self.bus.get::<u16>(self.regs.pc()).u32();
        } else {
            let time = self.bus.wait_time::<u32>(self.regs.pc(), NONSEQ | CODE);
            self.bus.tick(time as u64);
            self.pipeline[0] = self.bus.get::<u32>(self.regs.pc());

            self.regs.bump_pc(4);
            let time = self.bus.wait_time::<u32>(self.regs.pc(), SEQ | CODE);
            self.bus.tick(time as u64);
            self.pipeline[1] = self.bus.get::<u32>(self.regs.pc());
        };
        self.access_type = SEQ;
        self.block_ended = true;
    }

    /// Update the pipeline to be valid again, without wait states or actual
    /// reads
    pub fn revalidate_pipeline(&mut self) {
        if self.regs.is_flag(Thumb) {
            self.pipeline[0] = self.bus.get::<u16>(self.regs.pc() - Address::HW).u32();
            self.pipeline[1] = self.bus.get::<u16>(self.regs.pc()).u32();
        } else {
            self.pipeline[0] = self.bus.get::<u32>(self.regs.pc() - Address::WORD);
            self.pipeline[1] = self.bus.get::<u32>(self.regs.pc());
        }
    }

    #[inline]
    pub fn current_instruction_size(&self) -> u32 {
        // 4 on ARM, 2 on THUMB
        4 - ((self.regs.is_flag(Thumb) as u32) << 1)
    }
}

impl<S: Bus> Default for Cpu<S> {
    fn default() -> Self {
        Self {
            bus: S::default(),
            debugger: Debugger::default(),

            regs: Registers::default(),
            pipeline: [0; 2],
            access_type: NONSEQ,
            is_halted: false,
            block_ended: false,
            cache: Cache::default(),
            waitloop: WaitloopData::default(),

            ime: false,
            ie: 0,
            if_: 0,
        }
    }
}
