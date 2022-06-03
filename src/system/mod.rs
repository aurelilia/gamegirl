use serde::{Deserialize, Serialize};
use std::mem;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::numutil::NumExt;
use crate::system::cpu::{Cpu, Interrupt};
use crate::system::io::addr::{IF, KEY1};
use crate::system::io::cartridge::Cartridge;
use crate::system::io::Mmu;

use self::debugger::Debugger;

pub mod cpu;
pub mod debugger;
pub mod io;

const T_CLOCK_HZ: usize = 4194304;
const M_CLOCK_HZ: f32 = T_CLOCK_HZ as f32 / 4.0;

/// The system and it's state.
/// Represents the entire console.
#[derive(Deserialize, Serialize)]
pub struct GameGirl {
    pub cpu: Cpu,
    pub mmu: Mmu,
    #[serde(skip)]
    #[serde(default)]
    pub debugger: Arc<Debugger>,

    /// Shift of t clocks, which is different in CGB double speed mode. Regular: 2, CGB 2x: 1.
    t_shift: u8,
    /// Temporary for keeping track of how many clocks elapsed in [advance_delta].
    clock: usize,
    /// If the system is running. If false, any calls to [advance_delta] and [produce_samples] do nothing.
    pub running: bool,
    /// If there is a ROM loaded / cartridge inserted.
    pub rom_loaded: bool,
    /// If the audio samples produced by [produce_samples] should be in reversed order.
    /// `true` while rewinding.
    pub invert_audio_samples: bool,
}

impl GameGirl {
    /// Advance the system clock by the given delta in seconds.
    /// Might advance a few clocks more; especially if a GDMA transfer
    /// occurs at the wrong time.
    pub fn advance_delta(&mut self, delta: f32) {
        if !self.running {
            return;
        }
        self.clock = 0;
        let target = (M_CLOCK_HZ * delta) as usize;
        while self.clock < target {
            if self.debugger.breakpoint_hit.load(Ordering::Relaxed) {
                self.debugger.breakpoint_hit.store(false, Ordering::Relaxed);
                self.running = false;
                break;
            }
            self.advance();
        }
    }

    /// Produce the next `count` amount of audio samples.
    /// Returns `None` if the system is not currently running
    /// and no audio should be played.
    pub fn produce_samples(&mut self, count: usize) -> Option<Vec<f32>> {
        if !self.running {
            return None;
        }

        while self.mmu.apu.buffer.len() < count {
            if self.debugger.breakpoint_hit.load(Ordering::Relaxed) {
                self.debugger.breakpoint_hit.store(false, Ordering::Relaxed);
                self.running = false;
                return None;
            }
            self.advance();
        }
        let mut samples = mem::take(&mut self.mmu.apu.buffer);
        for sample in samples.drain(count..) {
            self.mmu.apu.buffer.push(sample);
        }

        if self.invert_audio_samples {
            samples.reverse();
        }
        Some(samples)
    }

    /// Advance the system by a single CPU instruction.
    pub fn advance(&mut self) {
        Cpu::exec_next_inst(self)
    }

    /// Advance the MMU, which is everything except the CPU.
    fn advance_clock(&mut self, m_cycles: usize) {
        let t_cycles = m_cycles << self.t_shift;
        Mmu::step(self, m_cycles, t_cycles);
        self.clock += m_cycles
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
        self.mmu = self.mmu.reset();
        self.t_shift = 2;
    }

    /// Create a save state that can be loaded with [load_state].
    /// It is zstd-compressed bincode.
    /// PPU display output and the cartridge are not stored.
    pub fn save_state(&self) -> Vec<u8> {
        if cfg!(target_arch = "wasm32") {
            // Currently crashes when loading...
            return vec![];
        }
        let mut dest = vec![];
        let mut writer = zstd::stream::Encoder::new(&mut dest, 3).unwrap();
        bincode::serialize_into(&mut writer, self).unwrap();
        writer.finish().unwrap();
        dest
    }

    /// Load a state produced by [save_state].
    /// Will restore the current cartridge and debugger.
    pub fn load_state(&mut self, state: &[u8]) {
        if cfg!(target_arch = "wasm32") {
            // Currently crashes...
            return;
        }
        let decoder = zstd::stream::Decoder::new(state).unwrap();
        let old_self = mem::replace(self, bincode::deserialize_from(decoder).unwrap());
        self.debugger = old_self.debugger;
        self.mmu.cart.rom = old_self.mmu.cart.rom;
        self.mmu.bootrom = old_self.mmu.bootrom;
    }

    /// Create a new console with no cartridge loaded.
    pub fn new() -> Self {
        let debugger = Arc::new(Debugger::default());
        Self {
            cpu: Cpu::default(),
            mmu: Mmu::new(debugger.clone()),
            debugger,

            t_shift: 2,
            clock: 0,
            running: false,
            rom_loaded: false,
            invert_audio_samples: false,
        }
    }

    /// Load the given cartridge.
    /// `reset` indicates if the system should be reset before loading.
    pub fn load_cart(&mut self, cart: Cartridge, config: &GGOptions, reset: bool) {
        if reset {
            let dbg = self.debugger.clone();
            *self = Self::new();
            self.debugger = dbg.clone();
            self.mmu.debugger = dbg;
        }
        self.mmu.load_cart(cart, config);
        self.running = true;
        self.rom_loaded = true;
    }

    /// Create a system with a cart already loaded.
    pub fn with_cart(rom: Vec<u8>) -> Self {
        let mut gg = Self::new();
        gg.load_cart(Cartridge::from_rom(rom), &GGOptions::default(), false);
        gg
    }
}

/// Configuration used when initializing the system.
#[derive(Default, Serialize, Deserialize)]
pub struct GGOptions {
    /// How to handle CGB mode.
    pub mode: CgbMode,
}

/// How to handle CGB mode depending on cart compatibility.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
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

impl Default for CgbMode {
    fn default() -> Self {
        Self::Prefer
    }
}
