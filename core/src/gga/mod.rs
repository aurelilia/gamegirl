use std::mem;

use audio::Apu;
use cartridge::Cartridge;
use cpu::Cpu;
use graphics::Ppu;
use memory::Memory;
use serde::{Deserialize, Serialize};

use crate::{
    common::{self, EmulateOptions, SystemConfig},
    debugger::Debugger,
    gga::{
        addr::{KEYINPUT, SOUNDBIAS},
        audio::SAMPLE_EVERY_N_CLOCKS,
        cpu::registers::Flag,
        dma::Dmas,
        scheduling::{AdvEvent, ApuEvent, PpuEvent},
        timer::Timers,
    },
    numutil::NumExt,
    scheduler::Scheduler,
    Colour,
};

pub mod addr;
mod audio;
mod cartridge;
pub mod cpu;
mod dma;
mod graphics;
mod input;
mod memory;
mod scheduling;
mod timer;

pub const CPU_CLOCK: f32 = 2u32.pow(24) as f32;

pub type GGADebugger = Debugger<u32>;

/// Console struct representing a GGA. Contains all state and is used for system
/// emulation.
#[derive(Deserialize, Serialize)]
pub struct GameGirlAdv {
    pub cpu: Cpu,
    pub memory: Memory,
    pub ppu: Ppu,
    pub apu: Apu,
    pub dma: Dmas,
    pub timers: Timers,
    pub cart: Cartridge,

    scheduler: Scheduler<AdvEvent>,
    pub options: EmulateOptions,
    pub config: SystemConfig,

    #[serde(skip)]
    #[serde(default)]
    pub debugger: GGADebugger,
    /// Temporary used by [advance_delta]. Will be true until the scheduled
    /// PauseEmulation event fires.
    ticking: bool,
}

impl GameGirlAdv {
    /// Advance the system clock by the given delta in seconds.
    /// Might advance a few clocks more.
    pub fn advance_delta(&mut self, delta: f32) {
        if !self.options.running {
            return;
        }

        let target = (CPU_CLOCK * delta * self.options.speed_multiplier as f32) as u32;
        self.scheduler.schedule(AdvEvent::PauseEmulation, target);

        self.ticking = true;
        while self.options.running && self.ticking {
            self.advance();
        }
    }

    /// Step until the PPU has finished producing the current frame.
    /// Only used for rewinding since it causes audio desync very easily.
    pub fn produce_frame(&mut self) -> Option<Vec<Colour>> {
        while self.options.running && self.ppu.last_frame == None {
            self.advance();
        }
        self.ppu.last_frame.take()
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

    /// Step forward the emulated console including all subsystems.
    pub fn advance(&mut self) {
        Cpu::exec_next_inst(self)
    }

    /// Advance everything but the CPU by a clock cycle.
    fn advance_clock(&mut self) {
        while let Some(event) = self.scheduler.get_next_pending() {
            event.kind.dispatch(self, event.late_by);
        }
    }

    /// Add wait cycles, which advance the system besides the CPU.
    #[inline]
    fn add_wait_cycles(&mut self, count: u16) {
        self.scheduler.advance(count.u32());
    }

    fn reg(&self, idx: u32) -> u32 {
        self.cpu.reg(idx)
    }

    fn low(&self, idx: u16) -> u32 {
        self.cpu.low(idx)
    }

    pub fn get_inst_mnemonic(&self, ptr: u32) -> String {
        if self.cpu.flag(Flag::Thumb) {
            let inst = self.get_hword(ptr);
            Self::get_mnemonic_thumb(inst)
        } else {
            let inst = self.get_word(ptr);
            Self::get_mnemonic_arm(inst)
        }
    }

    /// Create a save state that can be loaded with [load_state].
    pub fn save_state(&self) -> Vec<u8> {
        common::serialize(self, self.config.compress_savestates)
    }

    /// Load a state produced by [save_state].
    /// Will restore the current cartridge and debugger.
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

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        let save = old_self.cart.make_save();
        self.cart.load_rom(old_self.cart.rom);
        if let Some(save) = save {
            self.cart.load_save(save);
        }

        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
        self.init_memory();
    }

    /// Reset the console; like power-cycling a real one.
    pub fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
        Cpu::pipeline_stall(self);
    }

    pub fn skip_bootrom(&mut self) {
        self.cpu.pc = 0x0800_0000;
        self.cpu.cpsr = 0x1F;
        self.cpu.sp[1] = 0x0300_7F00;
        self.cpu.sp[3] = 0x0300_7F00;
        self.cpu.sp[5] = 0x0300_7F00;
    }
}

impl Default for GameGirlAdv {
    fn default() -> Self {
        let mut gg = Self {
            cpu: Cpu::default(),
            memory: Memory::default(),
            ppu: Ppu::default(),
            apu: Apu::default(),
            dma: Dmas::default(),
            timers: Timers::default(),
            cart: Cartridge::default(),

            scheduler: Scheduler::default(),
            options: EmulateOptions::default(),
            config: SystemConfig::default(),
            debugger: GGADebugger::default(),
            ticking: true,
        };

        // Initialize various IO registers
        gg[KEYINPUT] = 0x3FF;
        gg[SOUNDBIAS] = 0x200;

        // Initialize scheduler events
        gg.scheduler
            .schedule(AdvEvent::PpuEvent(PpuEvent::HblankStart), 960);
        Apu::init_scheduler(&mut gg);
        gg.scheduler
            .schedule(AdvEvent::ApuEvent(ApuEvent::Sequencer), 0x8000);
        gg.scheduler.schedule(
            AdvEvent::ApuEvent(ApuEvent::PushSample),
            SAMPLE_EVERY_N_CLOCKS,
        );

        gg
    }
}

/// Enum for the types of memory accesses; either sequential
/// or non-sequential. The numbers assigned to the variants are
/// to speed up reading the wait times in `memory.rs`.
#[derive(Copy, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Access {
    Seq = 0,
    NonSeq = 16,
}
