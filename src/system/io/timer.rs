use crate::numutil::NumExt;
use crate::system::cpu::Interrupt;
use crate::system::io::addr::*;
use crate::system::io::Mmu;
use crate::system::GameGirl;
use serde::Deserialize;
use serde::Serialize;

/// Timer available on DMG and CGB.
#[derive(Deserialize, Serialize)]
pub struct Timer {
    div: usize,
    counter_timer: usize,
    interrupt_next: bool,

    control: u8,
    counter_running: bool,
    counter_divider: usize,
    counter_bit: u16,
}

impl Timer {
    pub fn step(gg: &mut GameGirl, t_cycles: usize) {
        let mut tim = gg.timer();
        tim.div += t_cycles;
        if tim.interrupt_next {
            tim.interrupt_next = false;
            gg.mmu[TIMA] = gg.mmu[TMA];
            gg.request_interrupt(Interrupt::Timer);
        }

        let mut tima = gg.mmu[TIMA].u16();
        let mut tim = gg.timer(); // Work around borrow checker
        if tim.counter_running {
            tim.counter_timer += t_cycles;
            while tim.counter_timer >= tim.counter_divider {
                tim.counter_timer -= tim.counter_divider;
                tima += 1;
                if tima > 0xFF {
                    tim.interrupt_next = true;
                }
            }
            gg.mmu[TIMA] = tima.u8();
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            DIV => (self.div >> 8) as u8,
            TAC => self.control | 0xF8,
            _ => 0xFF,
        }
    }

    pub fn write(mmu: &mut Mmu, addr: u16, value: u8) {
        match addr {
            DIV => {
                let prev = mmu.timer.div.is_bit(mmu.timer.counter_bit);
                mmu.timer.div = 0;
                mmu.timer.counter_timer = 0;
                if prev {
                    let mut tima = mmu[TIMA].u16();
                    tima += 1;
                    if tima > 0xFF {
                        mmu.timer.interrupt_next = true;
                    }
                    mmu[TIMA] = tima.u8();
                }
            }
            TAC => {
                mmu.timer.control = value & 7;
                mmu.timer.counter_running = mmu.timer.control.is_bit(2);
                mmu.timer.counter_divider = match mmu.timer.control & 3 {
                    0 => 1024, // 4K
                    1 => 16,   // 256K
                    2 => 64,   // 64K
                    _ => 256,  // 16K (3)
                };
                mmu.timer.counter_bit = match mmu.timer.control & 3 {
                    0 => 9, // 4K
                    1 => 3, // 256K
                    2 => 5, // 64K
                    _ => 7, // 16K (3)
                };
            }
            _ => (),
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            div: 0,
            counter_timer: 0,
            interrupt_next: false,
            control: 0,
            counter_running: false,
            counter_divider: 1024,
            counter_bit: 9,
        }
    }
}

impl GameGirl {
    #[inline]
    fn timer(&mut self) -> &mut Timer {
        &mut self.mmu.timer
    }
}
