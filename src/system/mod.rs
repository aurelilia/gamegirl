use std::mem;
use std::sync::{Arc, RwLock};

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

pub struct GameGirl {
    pub cpu: Cpu,
    pub mmu: Mmu,
    pub debugger: Option<Arc<RwLock<Debugger>>>,

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
            self.advance();
        }
    }

    pub fn produce_samples(&mut self, count: usize) -> Option<Vec<f32>> {
        if !self.running {
            return None;
        }

        while self.mmu.apu.buffer.len() < count {
            self.advance();
        }
        let mut samples = mem::take(&mut self.mmu.apu.buffer);
        while samples.len() > count {
            self.mmu.apu.buffer.push(samples.pop().unwrap());
        }
        Some(samples)
    }

    fn advance(&mut self) {
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

    pub fn new(debugger: Option<Arc<RwLock<Debugger>>>) -> Self {
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

    pub fn load_cart(&mut self, cart: Vec<u8>, reset: bool) {
        if reset {
            *self = Self::new(self.debugger.clone());
        }
        let cart = Cartridge::from_rom(cart);
        self.mmu.load_cart(cart);
        self.running = true;
        self.rom_loaded = true;
    }

    pub fn with_cart(rom: Vec<u8>, debugger: Option<Arc<RwLock<Debugger>>>) -> Self {
        let mut gg = Self::new(debugger);
        gg.load_cart(rom, false);
        gg
    }
}
