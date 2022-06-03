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

#[derive(Deserialize, Serialize)]
pub struct GameGirl {
    pub cpu: Cpu,
    pub mmu: Mmu,
    #[serde(skip)]
    #[serde(default)]
    pub debugger: Arc<Debugger>,

    t_shift: u8,
    clock: usize,
    pub running: bool,
    pub rom_loaded: bool,
}

impl GameGirl {
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
        Some(samples)
    }

    pub fn advance(&mut self) {
        Cpu::exec_next_inst(self)
    }

    fn advance_clock(&mut self, m_cycles: usize) {
        let t_cycles = m_cycles << self.t_shift;
        Mmu::step(self, m_cycles, t_cycles);
        self.clock += m_cycles
    }

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

    pub fn reset(&mut self) {
        self.cpu = Cpu::default();
        self.mmu = self.mmu.reset();
        self.t_shift = 2;
    }

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
        }
    }

    pub fn load_cart(&mut self, cart: Cartridge, reset: bool) {
        if reset {
            let dbg = self.debugger.clone();
            *self = Self::new();
            self.debugger = dbg.clone();
            self.mmu.debugger = dbg;
        }
        self.mmu.load_cart(cart);
        self.running = true;
        self.rom_loaded = true;
    }

    pub fn with_cart(rom: Vec<u8>) -> Self {
        let mut gg = Self::new();
        gg.load_cart(Cartridge::from_rom(rom), false);
        gg
    }
}
