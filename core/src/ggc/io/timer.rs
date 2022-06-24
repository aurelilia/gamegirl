use crate::{
    ggc::{
        cpu::Interrupt,
        io::{addr::*, scheduling::GGEvent, Mmu},
        GameGirl,
    },
    numutil::NumExt,
};
use serde::{Deserialize, Serialize};

/// Timer available on DMG and CGB.
#[derive(Deserialize, Serialize)]
pub struct Timer {
    div_start: u32,
    scheduled_at: u32,
    control: u8,
    counter_running: bool,
    counter_divider: u16,
    counter_bit: u16,
}

impl Timer {
    pub fn on_overflow(gg: &mut GameGirl, late_by: u32) {
        gg.mmu[TIMA] = gg.mmu[TMA];
        gg.request_interrupt(Interrupt::Timer);
        gg.mmu.timer.scheduled_at = gg.mmu.scheduler.now();
        let time =
            Self::next_overflow_time(gg.mmu[TMA], gg.timer().counter_divider) / gg.mmu.speed.u32();
        gg.mmu.scheduler.schedule(
            GGEvent::TimerOverflow,
            time.checked_sub(late_by).unwrap_or(1),
        );
    }

    fn next_overflow_time(tma: u8, divider: u16) -> u32 {
        divider.u32() * (0xFF - tma.u32())
    }

    pub fn read(mmu: &Mmu, addr: u16) -> u8 {
        match addr {
            DIV => (mmu.scheduler.now() >> 8) as u8,
            TAC => mmu.timer.control | 0xF8,

            TIMA => {
                if mmu.timer.counter_running {
                    let time_elapsed = mmu.scheduler.now() - mmu.timer.scheduled_at;
                    mmu[TIMA]
                        + ((time_elapsed.u16() * mmu.speed.u16()) / mmu.timer.counter_divider).u8()
                } else {
                    mmu[TIMA]
                }
            }

            _ => 0xFF,
        }
    }

    pub fn write(mmu: &mut Mmu, addr: u16, value: u8) {
        match addr {
            DIV => {
                let prev = Self::div(mmu).is_bit(mmu.timer.counter_bit);
                mmu.timer.div_start = mmu.scheduler.now();
                if prev {
                    mmu.scheduler.pull_forward(GGEvent::TimerOverflow, 4);
                    mmu.timer.scheduled_at -= 4;
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

                mmu.scheduler.cancel(GGEvent::TimerOverflow);
                if mmu.timer.counter_running {
                    mmu.timer.scheduled_at = mmu.scheduler.now();
                    let time = Self::next_overflow_time(mmu[TMA], mmu.timer.counter_divider)
                        / mmu.speed.u32();
                    mmu.scheduler.schedule(GGEvent::TimerOverflow, time);
                }
            }
            _ => (),
        }
    }

    fn div(mmu: &mut Mmu) -> u16 {
        (mmu.scheduler.now() - mmu.timer.div_start).u16()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            div_start: 0,
            scheduled_at: 0,
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
