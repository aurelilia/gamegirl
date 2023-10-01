// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(adt_const_params)]
#![feature(const_mut_refs)]
#![allow(incomplete_features)]
#![allow(unused)]
#![allow(clippy::unused_self)]

use std::mem;

use common::{
    common_functions,
    components::{debugger::Debugger, scheduler::Scheduler, storage::GameSave},
    misc::{Button, EmulateOptions, SystemConfig},
    Colour, Core,
};

use crate::{apu::Apu, cpu::Cpu, gpu::Gpu, memory::Memory, scheduling::PsxEvent};

mod apu;
mod cpu;
mod gpu;
mod memory;
mod scheduling;

pub type PsxDebugger = Debugger<u32>;

/// System state representing entire console.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PlayStation {
    cpu: Cpu,
    ppu: Gpu,
    apu: Apu,
    memory: Memory,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub debugger: PsxDebugger,
    scheduler: Scheduler<PsxEvent>,

    pub options: EmulateOptions,
    pub config: SystemConfig,
    ticking: bool,
}

impl Core for PlayStation {
    common_functions!(1, PsxEvent::PauseEmulation, [640, 480]);

    fn advance(&mut self) {
        Cpu::execute_next(self);
    }

    fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
    }

    fn skip_bootrom(&mut self) {
        todo!();
    }

    fn set_button(&mut self, btn: Button, pressed: bool) {
        todo!();
    }

    fn make_save(&self) -> Option<GameSave> {
        todo!();
    }
}

impl PlayStation {
    /// Advance the scheduler, which controls everything except the CPU.
    fn advance_clock(&mut self, cycles: u32) {
        self.scheduler.advance(cycles);
        while let Some(event) = self.scheduler.get_next_pending() {
            event.kind.dispatch(self, event.late_by);
        }
    }

    /// Reset the console, while keeping the current cartridge inserted.
    pub fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
    }

    pub fn skip_bootrom(&mut self) {
        todo!()
    }
}
