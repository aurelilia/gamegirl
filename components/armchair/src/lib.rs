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
pub mod state;
mod thumb;
pub mod tracing;

use core::{
    mem,
    ops::{Deref, DerefMut},
};

use common::numutil::NumExt;
pub use exceptions::*;
use interface::Bus;
pub use memory::{access, Access, Address, RelativeOffset};
use optimizations::Optimizations;
pub use state::CpuState;

use crate::{interface::RwType, state::Flag::Thumb};

// TODO initial pipeline fill

/// Represents the CPU of the console.
/// It is generic over the system used; see `interface.rs`.
#[repr(C)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cpu<S: Bus> {
    pub state: CpuState,
    pub bus: S,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    pub(crate) opt: Optimizations<S>,
}

impl<S: Bus> Cpu<S> {
    /// Advance emulation.
    #[inline]
    pub fn continue_running(&mut self) {
        let pc = self.state.pc();
        if !self.bus.debugger().should_execute(pc.0) {
            return;
        }

        self.interpret_next_instruction();
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

    pub fn new(bus: S) -> Self {
        Self {
            bus,
            state: Default::default(),
            opt: Default::default(),
        }
    }
}

#[repr(C)]
pub struct FullBus<S: Bus> {
    cpu: CpuState,
    bus: S,
}

impl<S: Bus> FullBus<S> {
    /// Build a full bus from it's parts.
    /// The 2 parts _must_ be from the same system, otherwise this _will_ panic!
    pub fn from_parts<'s>(cpu: &'s mut CpuState, bus: &'s mut S) -> &'s mut Self {
        let transmuted: &'s mut Self = unsafe { mem::transmute(cpu) };
        debug_assert_eq!(bus as *const _, (&transmuted.bus) as *const _);
        transmuted
    }
}

impl<S: Bus> Deref for FullBus<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.bus
    }
}

impl<S: Bus> DerefMut for FullBus<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bus
    }
}

impl<'c, S: Bus> From<&'c Cpu<S>> for &FullBus<S> {
    fn from(value: &'c Cpu<S>) -> Self {
        unsafe { mem::transmute(value) }
    }
}

impl<'c, S: Bus> From<&'c mut Cpu<S>> for &mut FullBus<S> {
    fn from(value: &'c mut Cpu<S>) -> Self {
        unsafe { mem::transmute(value) }
    }
}
