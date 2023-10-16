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

use std::{mem, path::PathBuf, sync::Arc};

use addr::{DMACTRL, GPUSTAT};
use common::{
    common_functions,
    components::{
        debugger::Debugger,
        scheduler::Scheduler,
        storage::{GameSave, Storage},
    },
    misc::{Button, EmulateOptions, SystemConfig},
    produce_samples_buffered, Colour, Core,
};
use glow::Context;
use iso::Iso;

use crate::{apu::Apu, cpu::Cpu, gpu::Gpu, memory::Memory, scheduling::PsxEvent};

mod addr;
mod apu;
mod cpu;
mod dma;
mod gpu;
mod iso;
mod memory;
mod scheduling;

const CPU_CLOCK: usize = 33868800;
const GPU_CLOCK: usize = 53222400;
const FRAME_CLOCK: i32 = 571212;
const SAMPLE_CLOCK: i32 = 768;

pub type PsxDebugger = Debugger<u32>;

/// System state representing entire console.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PlayStation {
    pub cpu: Cpu,
    pub ppu: Gpu,
    pub apu: Apu,
    pub memory: Memory,
    pub iso: Iso,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub debugger: PsxDebugger,
    scheduler: Scheduler<PsxEvent>,

    pub options: EmulateOptions,
    pub config: SystemConfig,
    ticking: bool,
}

impl Core for PlayStation {
    common_functions!(CPU_CLOCK, PsxEvent::PauseEmulation, [1024, 512]);
    produce_samples_buffered!(48000);

    fn advance(&mut self) {
        Cpu::execute_next(self);
    }

    fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
    }

    fn skip_bootrom(&mut self) {
        log::error!("PSX does not support skipping bootrom yet");
    }

    fn set_button(&mut self, btn: Button, pressed: bool) {
        log::error!("PSX does not support buttons yet");
    }

    fn make_save(&self) -> Option<GameSave> {
        log::error!("PSX does not support making a save yet");
        None
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

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
    }

    /// Create a system with an ISO already loaded.
    pub fn with_iso(
        iso: Vec<u8>,
        path: Option<PathBuf>,
        config: &SystemConfig,
        ogl_ctx: Option<Arc<Context>>,
        ogl_tex_id: u32,
    ) -> Box<Self> {
        let mut iso = Iso { raw: iso };
        if let Some(save) = Storage::load(path, iso.title()) {
            todo!()
        }

        let mut ps = Box::<Self>::default();
        ps.ppu.init(ogl_ctx, ogl_tex_id);

        // DMA
        ps[DMACTRL] = 0x07654321;
        // Unknown DMA registers with fixed values
        ps[0x0F8] = 0x7FFAC68B;
        ps[0x0FC] = 0x00FFFFF7;

        ps.scheduler.schedule(PsxEvent::OutputFrame, FRAME_CLOCK);
        ps.scheduler.schedule(PsxEvent::ProduceSample, SAMPLE_CLOCK);

        ps
    }
}
