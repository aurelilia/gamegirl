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

use common::numutil::NumExt;
pub use exceptions::*;
use interface::Bus;
pub use memory::{access, Access, Address, RelativeOffset};
use optimizations::Optimizations;
pub use state::CpuState;

use crate::{interface::RwType, state::Flag::Thumb};

/// Represents the CPU of the console.
/// It is generic over the system used; see `interface.rs`.
#[repr(C)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cpu<S: Bus> {
    pub state: CpuState,
    pub bus: S,
    #[cfg_attr(feature = "serde", serde(skip, default = "Optimizations::new::<S>"))]
    pub opt: Optimizations,
}

impl<S: Bus> Cpu<S> {
    /// Advance emulation.
    #[inline]
    pub fn continue_running(&mut self) {
        let pc = self.state.pc();
        if !self.bus.debugger().should_execute(pc.0) {
            return;
        }

        match self.opt.jit_block.take() {
            Some(jit) => {
                let jit = self.opt.table.get_jit(jit);
                jit.call(self);
                log::debug!("JIT block ended with PC at {}", self.state.pc());
                self.state.revalidate_pipeline(&mut self.bus);
            }
            None => self.interpret_next_instruction(),
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

    pub fn new(bus: S) -> Self {
        Self {
            bus,
            state: Default::default(),
            opt: Optimizations::new::<S>(),
        }
    }
}
