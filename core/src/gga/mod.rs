use crate::{
    common::{self, EmulateOptions},
    debugger::Debugger,
    gga::{addr::KEYINPUT, cpu::registers::Flag, dma::Dmas, timer::Timers},
    ggc::GGConfig,
    numutil::NumExt,
    Colour,
};
use audio::Apu;
use cartridge::Cartridge;
use cpu::Cpu;
use graphics::Ppu;
use memory::Memory;
use serde::{Deserialize, Serialize};
use std::{
    mem,
    sync::{atomic::Ordering, Arc},
};

pub mod addr;
mod audio;
mod cartridge;
pub mod cpu;
mod dma;
mod graphics;
mod input;
mod memory;
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

    pub options: EmulateOptions,
    pub config: GGConfig,

    #[serde(skip)]
    #[serde(default)]
    pub debugger: Arc<GGADebugger>,

    /// Temporary for counting cycles in `advance_delta`.
    #[serde(skip)]
    #[serde(default)]
    clock: usize,

    /// Temporary used during instruction execution that counts
    /// the amount of cycles the CPU has to wait until it can
    /// execute the next instruction.
    wait_cycles: u16,
}

impl GameGirlAdv {
    /// Advance the system clock by the given delta in seconds.
    /// Might advance a few clocks more.
    pub fn advance_delta(&mut self, delta: f32) {
        if !self.options.running {
            return;
        }
        self.clock = 0;
        let target = (CPU_CLOCK * delta * self.options.speed_multiplier as f32) as usize;
        while self.clock < target {
            if self.debugger.breakpoint_hit.load(Ordering::Relaxed) {
                self.debugger.breakpoint_hit.store(false, Ordering::Relaxed);
                self.options.running = false;
                break;
            }
            self.advance();
        }
    }

    /// Step until the PPU has finished producing the current frame.
    /// Only used for rewinding since it causes audio desync very easily.
    pub fn produce_frame(&mut self) -> Option<Vec<Colour>> {
        if !self.options.running {
            return None;
        }

        while self.ppu.last_frame == None {
            if self.debugger.breakpoint_hit.load(Ordering::Relaxed) {
                self.debugger.breakpoint_hit.store(false, Ordering::Relaxed);
                self.options.running = false;
                return None;
            }
            self.advance();
        }

        self.ppu.last_frame.take()
    }

    /// Produce the next audio samples and write them to the given buffer.
    /// Writes zeroes if the system is not currently running
    /// and no audio should be played.
    pub fn produce_samples(&mut self, samples: &mut [f32]) {
        // Disable this for now, we don't have a working APU yet.
        // if !self.options.running {
        samples.fill(0.0);
        return;
        // }

        let target = samples.len() * self.options.speed_multiplier;
        while self.apu.buffer.len() < target {
            if self.debugger.breakpoint_hit.load(Ordering::Relaxed) {
                self.debugger.breakpoint_hit.store(false, Ordering::Relaxed);
                self.options.running = false;
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
        self.clock += self.wait_cycles.us();
        Ppu::step(self, self.wait_cycles);
        Timers::step(self, self.wait_cycles);
        self.wait_cycles = 0;
    }

    /// Add wait cycles, which advance the system besides the CPU.
    fn add_wait_cycles(&mut self, count: u16) {
        self.wait_cycles = self.wait_cycles.wrapping_add(count);
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
        self.cart.rom = old_self.cart.rom;
        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
        self.init_memory();
    }

    /// Reset the console; like power-cycling a real one.
    pub fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
        Cpu::fix_prefetch(self);
    }
}

impl Default for GameGirlAdv {
    fn default() -> Self {
        let mut gg = Self {
            cpu: Cpu::default(),
            memory: Memory::default(),
            ppu: Ppu::default(),
            apu: Apu {
                buffer: Vec::with_capacity(1000),
            },
            // Meh...
            dma: Dmas::default(),
            timers: Timers::default(),
            cart: Cartridge::default(),

            options: EmulateOptions::default(),
            config: GGConfig::default(),
            debugger: Arc::new(GGADebugger::default()),
            clock: 0,
            wait_cycles: 0,
        };

        gg[KEYINPUT] = 0x3FF;

        // Skip bootrom for now
        gg.cpu.pc = 0x0800_0000;
        gg.cpu.cpsr = 0x1F;
        gg.cpu.sp[1] = 0x0300_7F00;
        gg.cpu.sp[3] = 0x0300_7F00;
        gg.cpu.sp[5] = 0x0300_7F00;
        gg
    }
}

/// Enum for the types of memory accesses; either sequential
/// or non-sequential.
#[derive(Copy, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Access {
    Seq = 0,
    NonSeq = 16,
}
