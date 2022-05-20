use crate::numutil::NumExt;
use crate::system::cpu::{Cpu, Interrupt};
use crate::system::io::addr::{IF, KEY1};
use crate::system::io::Mmu;

mod cpu;
pub mod io;

#[derive(Default)]
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
        self.cpu.sp += 2;
        self.mmu.read16(self.cpu.sp - 2)
    }

    fn push_stack(&mut self, value: u16) {
        self.cpu.sp -= 2;
        self.mmu.write16(self.cpu.sp, value)
    }
}
