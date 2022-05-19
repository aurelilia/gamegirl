use crate::system::cpu::Interrupt;
use crate::GameGirl;

pub struct Timer {
    div_cycle_count: usize,
    counter_timer: usize,
    interrupt_in: isize,
    counter: u16,
    modulo: u16,
    control: u16,
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
                tim.counter = tim.modulo;
                gg.request_interrupt(Interrupt::Timer);
            }
        }

        let mut tim = gg.timer(); // Work around borrow checker
        if tim.counter_running {
            tim.counter_timer += t_cycles;
            while tim.counter_timer >= tim.counter_divider {
                tim.counter_timer -= tim.counter_divider;
                tim.counter += 1;
                if tim.counter > 0xFF {
                    tim.interrupt_in = 4;
                }
            }
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            div_cycle_count: 0,
            counter_timer: 0,
            interrupt_in: 0,
            counter: 0,
            modulo: 0,
            control: 0,
            counter_running: false,
            counter_divider: 1024,
        }
    }
}
