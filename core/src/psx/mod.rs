use std::mem;

use serde::{Deserialize, Serialize};

use crate::{
    common,
    common::{EmulateOptions, SystemConfig},
    debugger::Debugger,
    psx::{apu::Apu, cpu::Cpu, gpu::Gpu, scheduling::PsxEvent},
    scheduler::Scheduler,
    Colour,
};

mod apu;
mod cpu;
mod gpu;
mod memory;
mod scheduling;

pub type PsxDebugger = Debugger<u32>;

/// System state representing entire console.
#[derive(Deserialize, Serialize)]
pub struct PlayStation {
    cpu: Cpu,
    gpu: Gpu,
    apu: Apu,

    #[serde(skip)]
    #[serde(default)]
    pub debugger: PsxDebugger,
    scheduler: Scheduler<PsxEvent>,

    pub options: EmulateOptions,
    pub config: SystemConfig,
}

impl PlayStation {
    /// Step until the GPU has finished producing the current frame.
    /// Only used for rewinding since it causes audio desync very easily.
    pub fn produce_frame(&mut self) -> Option<Vec<Colour>> {
        while self.options.running && self.gpu.last_frame == None {
            self.advance();
        }
        self.gpu.last_frame.take()
    }

    /// Produce the next audio samples and write them to the given buffer.
    /// Writes zeroes if the system is not currently running
    /// and no audio should be played.
    pub fn produce_samples(&mut self, samples: &mut [f32]) {
        if !self.options.running {
            samples.fill(0.0);
            return;
        }

        let target = samples.len() * self.options.speed_multiplier;
        while self.apu.buffer.len() < target {
            if !self.options.running {
                samples.fill(0.0);
                return;
            }
            self.advance();
        }

        let mut buffer = mem::take(&mut self.apu.buffer);
        if self.options.invert_audio_samples {
            // If rewinding, truncate and get rid of any excess samples to prevent
            // audio samples getting backed up
            for (src, dst) in buffer.into_iter().zip(samples.iter_mut().rev()) {
                *dst = src * self.config.volume;
            }
        } else {
            // Otherwise, store any excess samples back in the buffer for next time
            // while again not storing too many to avoid backing up.
            // This way can cause clipping if the console produces audio too fast,
            // however this is preferred to audio falling behind and eating
            // a lot of memory.
            for sample in buffer.drain(target..) {
                self.apu.buffer.push(sample);
            }
            self.apu.buffer.truncate(5_000);

            for (src, dst) in buffer
                .into_iter()
                .step_by(self.options.speed_multiplier)
                .zip(samples.iter_mut())
            {
                *dst = src * self.config.volume;
            }
        }
    }

    /// Advance the system by a single CPU instruction.
    pub fn advance(&mut self) {
        Cpu::execute_next(self)
    }

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

    /// Reset the console, while keeping the current cartridge inserted.
    pub fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
    }

    /// Create a save state that can be loaded with [load_state].
    pub fn save_state(&self) -> Vec<u8> {
        common::serialize(self, self.config.compress_savestates)
    }

    /// Load a state produced by [save_state].
    /// Will restore the current CD and debugger.
    pub fn load_state(&mut self, state: &[u8]) {
        if cfg!(target_arch = "wasm32") {
            // Currently crashes...
            return;
        }

        let old_self = mem::replace(
            self,
            common::deserialize(state, self.config.compress_savestates),
        );
        self.restore_from(old_self);
    }

    pub fn skip_bootrom(&mut self) {
        todo!()
    }
}

impl Default for PlayStation {
    fn default() -> Self {
        Self {
            cpu: Cpu::default(),
            gpu: Gpu::default(),
            apu: Apu::default(),
            debugger: Debugger::default(),
            scheduler: Scheduler::default(),
            options: EmulateOptions::default(),
            config: SystemConfig::default(),
        }
    }
}
