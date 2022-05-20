use crate::numutil::NumExt;
use crate::system::cpu::Interrupt;
use crate::system::io::addr::*;
use crate::GameGirl;

pub struct Timer {
    div_cycle_count: usize,
    counter_timer: usize,
    interrupt_in: isize,
    control: u8,
    counter_running: bool,
    counter_divider: usize,
}

impl Timer {
    pub fn step(gg: &mut GameGirl, t_cycles: usize) {
        let mut tim = gg.timer();
        tim.div_cycle_count += t_cycles;
        if tim.interrupt_in > 0 {
            tim.interrupt_in -= t_cycles as isize;
            if tim.interrupt_in <= 0 {
                gg.mmu[TIMA] = gg.mmu[TMA];
                gg.request_interrupt(Interrupt::Timer);
            }
        }

        let tima = gg.mmu[TIMA];
        let mut tim = gg.timer(); // Work around borrow checker
        if tim.counter_running {
            tim.counter_timer += t_cycles;
            let value = tima.us() + (tim.counter_timer / tim.counter_divider);
            tim.counter_timer %= tim.counter_divider;
            if value >= 0xFF {
                tim.interrupt_in = 4;
            }
            gg.mmu[TIMA] = value as u8;
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            DIV => (self.div_cycle_count >> 8) as u8,
            TAC => self.control | 0xF8,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            DIV => {
                self.div_cycle_count = 0;
                self.counter_timer = 0;
            }
            TAC => {
                self.control = (value & 7);
                self.counter_running = self.control.is_bit(2);
                self.counter_divider = match self.control & 3 {
                    0 => 1024, // 4K
                    1 => 16,   // 256K
                    2 => 64,   // 64K
                    _ => 256,  // 16K (3)
                }
            }
            _ => (),
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            div_cycle_count: 0,
            counter_timer: 0,
            interrupt_in: 0,
            control: 0,
            counter_running: false,
            counter_divider: 1024,
        }
    }
}
