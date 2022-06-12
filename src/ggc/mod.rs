use crate::common::{self, EmulateOptions};
use crate::Colour;
use serde::{Deserialize, Serialize};
use std::mem;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::ggc::cpu::{Cpu, Interrupt};
use crate::ggc::io::addr::{IF, KEY1};
use crate::ggc::io::cartridge::Cartridge;
use crate::ggc::io::Mmu;
use crate::numutil::NumExt;

use crate::debugger::Debugger;

pub mod cpu;
pub mod io;

const T_CLOCK_HZ: usize = 4194304;
const M_CLOCK_HZ: f32 = T_CLOCK_HZ as f32 / 4.0;

pub type GGDebugger = Debugger<u16>;

/// The system and it's state.
/// Represents the entire console.
#[derive(Deserialize, Serialize)]
pub struct GameGirl {
    pub cpu: Cpu,
    pub mmu: Mmu,
    #[serde(skip)]
    #[serde(default)]
    pub debugger: Arc<GGDebugger>,
    pub config: GGConfig,

    /// Shift of t-clocks, which is different in CGB double speed mode. Regular: 2, CGB 2x: 1.
    t_shift: u8,
    /// Temporary for keeping track of how many clocks elapsed in [advance_delta].
    #[serde(skip)]
    #[serde(default)]
    clock: usize,
    /// Emulation options.
    pub options: EmulateOptions,
}

impl GameGirl {
    /// Advance the system clock by the given delta in seconds.
    /// Might advance a few clocks more; especially if a GDMA transfer
    /// occurs at the wrong time.
    pub fn advance_delta(&mut self, delta: f32) {
        if !self.options.running {
            return;
        }
        self.clock = 0;
        let target = (M_CLOCK_HZ * delta * self.options.speed_multiplier as f32) as usize;
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

        while self.mmu.ppu.last_frame == None {
            if self.debugger.breakpoint_hit.load(Ordering::Relaxed) {
                self.debugger.breakpoint_hit.store(false, Ordering::Relaxed);
                self.options.running = false;
                return None;
            }
            self.advance();
        }

        self.mmu.ppu.last_frame.take()
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
        while self.mmu.apu.buffer.len() < target {
            if self.debugger.breakpoint_hit.load(Ordering::Relaxed) {
                self.debugger.breakpoint_hit.store(false, Ordering::Relaxed);
                self.options.running = false;
                samples.fill(0.0);
                return;
            }
            self.advance();
        }

        let mut buffer = mem::take(&mut self.mmu.apu.buffer);
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
                self.mmu.apu.buffer.push(sample);
            }
            self.mmu.apu.buffer.truncate(5_000);

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
        Cpu::exec_next_inst(self)
    }

    /// Advance the MMU, which is everything except the CPU.
    /// Should not advance by more than 7 cycles at once.3
    fn advance_clock(&mut self, m_cycles: u16) {
        let t_cycles = m_cycles << self.t_shift;
        Mmu::step(self, m_cycles, t_cycles);
        self.clock += m_cycles.us();
    }

    /// Switch between CGB 2x and normal speed mode.
    fn switch_speed(&mut self) {
        self.t_shift = if self.t_shift == 2 { 1 } else { 2 };
        self.mmu[KEY1] = (self.t_shift & 1) << 7;
        for _ in 0..=1024 {
            self.advance_clock(2)
        }
    }

    fn request_interrupt(&mut self, ir: Interrupt) {
        self.mmu[IF] = self.mmu[IF].set_bit(ir.to_index(), true) as u8;
    }

    fn arg8(&self) -> u8 {
        self.mmu.read(self.cpu.pc + 1)
    }

    fn arg16(&self) -> u16 {
        self.mmu.read16(self.cpu.pc + 1)
    }

    fn pop_stack(&mut self) -> u16 {
        let val = self.mmu.read16(self.cpu.sp);
        self.cpu.sp = self.cpu.sp.wrapping_add(2);
        val
    }

    fn push_stack(&mut self, value: u16) {
        self.cpu.sp = self.cpu.sp.wrapping_sub(2);
        self.mmu.write16(self.cpu.sp, value)
    }

    /// Reset the console, while keeping the current cartridge inserted.
    pub fn reset(&mut self) {
        self.cpu = Cpu::default();
        self.mmu = self.mmu.reset(&self.config);
        self.t_shift = 2;
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
        self.debugger = old_self.debugger;
        self.mmu.cart.rom = old_self.mmu.cart.rom;
        self.options.frame_finished = old_self.options.frame_finished;
        self.mmu.bootrom = old_self.mmu.bootrom;
    }

    /// Load the given cartridge.
    /// `reset` indicates if the system should be reset before loading.
    pub fn load_cart(&mut self, cart: Cartridge, config: &GGConfig, reset: bool) {
        if reset {
            let old_self = mem::take(self);
            self.debugger = old_self.debugger.clone();
            self.mmu.debugger = old_self.debugger;
            self.options.frame_finished = old_self.options.frame_finished;
        }
        self.mmu.load_cart(cart, config);
        self.config = config.clone();
        self.options.running = true;
        self.options.rom_loaded = true;
    }

    /// Create a system with a cart already loaded.
    pub fn with_cart(rom: Vec<u8>) -> Self {
        let mut gg = Self::default();
        gg.load_cart(Cartridge::from_rom(rom), &GGConfig::default(), false);
        gg
    }
}

impl Default for GameGirl {
    fn default() -> Self {
        let debugger = Arc::new(GGDebugger::default());
        Self {
            cpu: Cpu::default(),
            mmu: Mmu::new(debugger.clone()),
            debugger,
            config: GGConfig::default(),

            t_shift: 2,
            clock: 0,
            options: EmulateOptions::default(),
        }
    }
}

/// Configuration used when initializing the system.
#[derive(Clone, Serialize, Deserialize)]
pub struct GGConfig {
    /// How to handle CGB mode.
    pub mode: CgbMode,
    /// If save states should be compressed.
    pub compress_savestates: bool,
    /// If CGB colours should be corrected.
    pub cgb_colour_correction: bool,
    /// Audio volume multiplier
    pub volume: f32,
}

impl Default for GGConfig {
    fn default() -> Self {
        Self {
            mode: CgbMode::Prefer,
            compress_savestates: false,
            cgb_colour_correction: false,
            volume: 0.5,
        }
    }
}

/// How to handle CGB mode depending on cart compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CgbMode {
    /// Always run in CGB mode, even when the cart does not support it.
    /// If it does not, it is run in DMG compatibility mode, just like on a
    /// real CGB.
    Always,
    /// If the cart has CGB support, run it as CGB; if not, don't.
    Prefer,
    /// Never run the cart in CGB mode unless it requires it.
    Never,
}
