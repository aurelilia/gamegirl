use crate::numutil::NumExt;
use crate::system::cpu::{Cpu, Interrupt};
use crate::system::io::addr::{IF, KEY1};
use crate::system::io::Mmu;

mod cpu;
pub mod io;

const T_CLOCK_HZ: f32 = 4194304.0;

pub struct GameGirl {
    pub cpu: Cpu,
    pub mmu: Mmu,

    pub t_multiplier: u8,
    pub clock: usize,
}

impl GameGirl {
    pub fn advance(&mut self) {
        Cpu::exec_next_inst(self)
    }

    pub fn advance_delta(&mut self, delta: f32) {
        self.clock = 0;
        let target = (T_CLOCK_HZ * delta) as usize;
        while self.clock < target {
            self.advance();
        }
    }

    fn advance_clock(&mut self, m_cycles: usize) {
        let t_cycles = m_cycles * 4;
        Mmu::step(self, t_cycles);
        self.clock += t_cycles
    }

    fn switch_speed(&mut self) {
        self.t_multiplier = if self.t_multiplier == 1 { 2 } else { 1 };
        self.mmu[KEY1] = 0;
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

    pub fn new(rom: Vec<u8>) -> Self {
        let mut gg = Self {
            cpu: Cpu::default(),
            mmu: Mmu::new(rom),

            t_multiplier: 1,
            clock: 0,
        };
        gg.mmu.init_high();
        gg
    }
}
