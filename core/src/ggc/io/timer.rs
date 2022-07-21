// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::{
    ggc::{
        io::{addr::*, scheduling::GGEvent},
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
    pub fn on_overflow(gg: &mut GameGirl, late_by: i32) {
        gg.timer.scheduled_at = gg.scheduler.now();
        let time = Self::next_overflow_time(gg) as i32;

        // Obscure behavior: It takes 1 M-cycle for the timer to actually reload
        gg[TIMA] = 0;
        gg.scheduler.schedule(GGEvent::TmaReload, 4 - late_by);

        gg.scheduler
            .schedule(GGEvent::TimerOverflow, time - late_by);
    }

    /// Get the time when the next overflow will occur, in t-cycles.
    fn next_overflow_time(gg: &GameGirl) -> u32 {
        let time = gg.timer.counter_divider.u32() * (0x100 - gg[TIMA].u32());
        // Account for already-running DIV causing less time for the first step
        let div = Self::div(gg) & (gg.timer.counter_divider - 1);
        // Account for 2x speed affecting the timer
        (time - div.u32()) / gg.speed.u32()
    }

    pub fn read(gg: &GameGirl, addr: u16) -> u8 {
        match addr {
            DIV => (Self::div(gg) >> 8).u8(),
            TAC => gg.timer.control | 0xF8,
            TIMA => Self::get_tima(gg),
            _ => 0xFF,
        }
    }

    fn get_tima(gg: &GameGirl) -> u8 {
        if gg.timer.counter_running {
            // Make sure we account for DIV being the timer's source of increases
            // properly, would count too little if not
            let time_elapsed = gg.scheduler.now() - gg.timer.scheduled_at;
            let time_ds = time_elapsed.u16() * gg.speed.u16();
            gg[TIMA] + (time_ds / gg.timer.counter_divider).u8()
        } else {
            gg[TIMA]
        }
    }

    pub fn write(gg: &mut GameGirl, addr: u16, value: u8) {
        match addr {
            DIV => {
                let prev = Self::div(gg);

                // Reset DIV.
                gg.timer.div_start = gg.scheduler.now();

                // Reschedule TIMA, it counts on DIV so the counting for this increment starts
                // anew If the DIV counter bit was set, increment TIMA
                // (increment on falling DIV edge)
                if gg.timer.counter_running {
                    gg[TIMA] = Self::get_tima(gg) + prev.bit(gg.timer.counter_bit).u8();
                    Self::reschedule(gg);
                }
            }
            TIMA => {
                gg[TIMA] = value;
                Self::reschedule(gg);
            }
            TAC => {
                // Update TIMA first of all
                gg[TIMA] = Self::get_tima(gg);

                // Update control values...
                gg.timer.control = value & 7;
                gg.timer.counter_running = gg.timer.control.is_bit(2);
                gg.timer.counter_divider = match gg.timer.control & 3 {
                    0 => 1024, // 4K
                    1 => 16,   // 256K
                    2 => 64,   // 64K
                    _ => 256,  // 16K (3)
                };
                gg.timer.counter_bit = match gg.timer.control & 3 {
                    0 => 9, // 4K
                    1 => 3, // 256K
                    2 => 5, // 64K
                    _ => 7, // 16K (3)
                };

                // Now reschedule if needed.
                Self::reschedule(gg);
            }
            _ => (),
        }
    }

    /// Calculate the current value of DIV.
    fn div(gg: &GameGirl) -> u16 {
        (gg.scheduler.now() - gg.timer.div_start).u16()
    }

    /// Reschedule the timer overflow event.
    fn reschedule(gg: &mut GameGirl) {
        gg.scheduler.cancel(GGEvent::TimerOverflow);
        if gg.timer.counter_running {
            gg.timer.scheduled_at =
                gg.scheduler.now() - (Self::div(gg) & (gg.timer.counter_divider - 1)).u32();
            gg.scheduler
                .schedule(GGEvent::TimerOverflow, Self::next_overflow_time(gg) as i32);
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

#[cfg(test)]
mod tests {
    use crate::{
        ggc::{
            io::{
                addr::{DIV, TAC, TIMA},
                timer::Timer,
            },
            GameGirl,
        },
        numutil::NumExt,
    };

    #[test]
    fn div_starts_at_0() {
        for_all_speeds(|gg, _| assert_eq!(0, Timer::read(gg, DIV)));
    }

    #[test]
    fn div_inc() {
        for_all_speeds(|gg, _| {
            gg.scheduler.advance(255);
            assert_eq!(0, Timer::read(gg, DIV));
            gg.scheduler.advance(1);
            assert_eq!(1, Timer::read(gg, DIV));
        });
    }

    #[test]
    fn div_reset() {
        for_all_speeds(|gg, _| {
            gg.scheduler.advance(512);
            assert_eq!(2, Timer::read(gg, DIV));
            Timer::write(gg, DIV, 234);
            assert_eq!(0, Timer::read(gg, DIV));

            gg.scheduler.advance(255);
            assert_eq!(0, Timer::read(gg, DIV));
            gg.scheduler.advance(1);
            assert_eq!(1, Timer::read(gg, DIV));
        });
    }

    #[test]
    fn starts_at_0() {
        for_all_speeds(|gg, _| assert_eq!(0, Timer::read(gg, TIMA)));
    }

    #[test]
    fn is_0_before_divider() {
        for_all_speeds(|gg, _| {
            gg.scheduler.advance(gg.timer.counter_divider.u32() - 1);
            assert_eq!(0, Timer::read(gg, TIMA));
        });
    }

    #[test]
    fn is_1_after_divider() {
        for_all_speeds(|gg, _| {
            gg.scheduler.advance(gg.timer.counter_divider.u32());
            assert_eq!(1, Timer::read(gg, TIMA));
        });
    }

    #[test]
    fn is_1_before_divider() {
        for_all_speeds(|gg, _| {
            gg.scheduler
                .advance((gg.timer.counter_divider.u32() * 2) - 1);
            assert_eq!(1, Timer::read(gg, TIMA));
        });
    }

    #[test]
    fn is_2_after_divider() {
        for_all_speeds(|gg, _| {
            gg.scheduler.advance(gg.timer.counter_divider.u32() * 2);
            assert_eq!(2, Timer::read(gg, TIMA));
        });
    }

    #[test]
    fn reschedule() {
        for_all_speeds(|gg, speed| {
            gg.scheduler.advance(gg.timer.counter_divider.u32() * 2);
            assert_eq!(2, Timer::read(gg, TIMA));

            Timer::write(gg, TAC, speed);
            gg.scheduler.advance(gg.timer.counter_divider.u32() * 2);
            assert_eq!(2, Timer::read(gg, TIMA));

            Timer::write(gg, TAC, speed | 4);
            gg.scheduler.advance(gg.timer.counter_divider.u32() * 2);
            assert_eq!(4, Timer::read(gg, TIMA));
        });
    }

    fn setup(run: bool, speed: u8) -> GameGirl {
        let mut gg = GameGirl::default();
        gg.init_high();
        Timer::write(&mut gg, TAC, speed | ((run as u8) << 2));
        gg
    }

    fn for_all_speeds(mut inner: impl FnMut(&mut GameGirl, u8)) {
        for speed in 0..4 {
            let mut gg = setup(true, speed);
            inner(&mut gg, speed);
        }
    }
}
