use serde::{Deserialize, Serialize};

use crate::{
    ggc::{
        cpu::Interrupt,
        io::{addr::*, scheduling::GGEvent, Mmu},
        GameGirl,
    },
    numutil::NumExt,
};

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
    /// Timer overflow event happened, reset TIMA, fire interrupt and
    /// reschedule.
    pub fn on_overflow(gg: &mut GameGirl, late_by: u32) {
        gg.mmu[TIMA] = gg.mmu[TMA];
        gg.request_interrupt(Interrupt::Timer);
        gg.mmu.timer.scheduled_at = gg.mmu.scheduler.now();
        let time = Self::next_overflow_time(&gg.mmu);

        // Obscure behavior: It takes 1 M-cycle for the timer to actually reload
        gg.mmu[TIMA] = 0;
        gg.mmu
            .scheduler
            .schedule(GGEvent::TmaReload, 4u32.saturating_sub(late_by));

        gg.mmu.scheduler.schedule(
            GGEvent::TimerOverflow,
            time.checked_sub(late_by).unwrap_or(1),
        );
    }

    /// Get the time when the next overflow will occur, in t-cycles.
    fn next_overflow_time(mmu: &Mmu) -> u32 {
        let time = mmu.timer.counter_divider.u32() * (0x100 - mmu[TIMA].u32());
        // Account for already-running DIV causing less time for the first step
        let div = Self::div(mmu) & (mmu.timer.counter_divider - 1);
        // Account for 2x speed affecting the timer
        (time - div.u32()) / mmu.speed.u32()
    }

    pub fn read(mmu: &Mmu, addr: u16) -> u8 {
        match addr {
            DIV => (Self::div(mmu) >> 8).u8(),
            TAC => mmu.timer.control | 0xF8,
            TIMA => Self::get_tima(mmu),
            _ => 0xFF,
        }
    }

    fn get_tima(mmu: &Mmu) -> u8 {
        if mmu.timer.counter_running {
            // Make sure we account for DIV being the timer's source of increases
            // properly, would count too little if not
            let time_elapsed = mmu.scheduler.now()
                - (mmu.timer.scheduled_at & !(mmu.timer.counter_divider.u32() - 1));
            let time_ds = time_elapsed.u16() * mmu.speed.u16();
            mmu[TIMA] + (time_ds / mmu.timer.counter_divider).u8()
        } else {
            mmu[TIMA]
        }
    }

    pub fn write(mmu: &mut Mmu, addr: u16, value: u8) {
        match addr {
            DIV => {
                let prev = Self::div(mmu);

                // Reset DIV.
                mmu.timer.div_start = mmu.scheduler.now();

                // Reschedule TIMA, it counts on DIV so the counting for this increment starts
                // anew If the DIV counter bit was set, increment TIMA
                // (increment on falling DIV edge)
                if mmu.timer.counter_running {
                    mmu[TIMA] = Self::get_tima(mmu) + prev.bit(mmu.timer.counter_bit).u8();
                    Self::reschedule(mmu);
                }
            }
            TIMA => {
                mmu[TIMA] = value;
                Self::reschedule(mmu);
            }
            TAC => {
                // Update TIMA first of all
                mmu[TIMA] = Self::get_tima(mmu);

                // Update control values...
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

                // Now reschedule if needed.
                Self::reschedule(mmu);
            }
            _ => (),
        }
    }

    /// Calculate the current value of DIV.
    fn div(mmu: &Mmu) -> u16 {
        (mmu.scheduler.now() - mmu.timer.div_start).u16()
    }

    /// Reschedule the timer overflow event.
    fn reschedule(mmu: &mut Mmu) {
        mmu.scheduler.cancel(GGEvent::TimerOverflow);
        if mmu.timer.counter_running {
            mmu.timer.scheduled_at = mmu.scheduler.now();
            mmu.scheduler
                .schedule(GGEvent::TimerOverflow, Self::next_overflow_time(mmu));
        }
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
