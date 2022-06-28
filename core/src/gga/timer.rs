use serde::{Deserialize, Serialize};

use crate::{
    gga::{
        addr::{SOUNDCNT_H, TM0CNT_H},
        audio::Apu,
        cpu::{Cpu, Interrupt},
        scheduling::AdvEvent,
        GameGirlAdv,
    },
    numutil::NumExt,
};

const DIVS: [u16; 4] = [1, 64, 256, 1024];

/// Timers on the GGA.
/// They run on the scheduler when in regular counting mode.
/// The scheduler variables have a bunch of small additions that work for some
/// reason, still not sure why. Some other timings that are inaccurate?
#[derive(Default, Deserialize, Serialize)]
pub struct Timers {
    /// Counter value. Used for cascading counters; for scheduled counters this
    /// will be the reload value (actual counter is calculated on read)
    counters: [u16; 4],
    /// The time the timer was scheduled, if it is on the scheduler.
    scheduled_at: [u32; 4],
}

impl Timers {
    /// Handle overflow of a scheduled timer.
    pub fn handle_overflow_event(gg: &mut GameGirlAdv, idx: u8, late_by: u32) {
        // Handle overflow
        let until_ov = Self::overflow(gg, idx);
        // Reschedule event
        // Edge case: with high reload and fast timers, sometimes (late_by > until_ov).
        // In this case, we simply schedule the next overflow event to be immediately.
        gg.timers.scheduled_at[idx.us()] = gg.scheduler.now() - late_by + 2;
        gg.scheduler.schedule(
            AdvEvent::TimerOverflow(idx),
            until_ov.saturating_sub(late_by),
        );
    }

    /// Read current time of the given timer. Might be a bit expensive, since
    /// time needs to be calculated for timers on the scheduler.
    pub fn time_read<const TIM: usize>(gg: &GameGirlAdv) -> u16 {
        let ctrl = gg[Self::hi_addr(TIM.u8())];
        let is_scheduled = ctrl.is_bit(7) && !ctrl.is_bit(2);

        if is_scheduled {
            // Is on scheduler, calculate current value
            let scaler = DIVS[(ctrl & 3).us()].u32();
            let elapsed = gg.scheduler.now() - gg.timers.scheduled_at[TIM];
            gg.timers.counters[TIM].wrapping_add((elapsed / scaler).u16())
        } else {
            // Either off or inc on overflow, just return current counter
            gg.timers.counters[TIM]
        }
    }

    /// Handle CTRL write by scheduling timer as appropriate.
    pub fn hi_write<const TIM: usize>(gg: &mut GameGirlAdv, addr: u32, new_ctrl: u16) {
        // Update current counter value first
        gg.timers.counters[TIM] = Self::time_read::<TIM>(gg);

        let old_ctrl = gg[addr];
        let was_scheduled = old_ctrl.is_bit(7) && !old_ctrl.is_bit(2);
        let is_scheduled = new_ctrl.is_bit(7) && !new_ctrl.is_bit(2);

        if was_scheduled {
            // Need to cancel current scheduled event
            gg.scheduler.cancel(AdvEvent::TimerOverflow(TIM.u8()));
        }
        if is_scheduled {
            if !was_scheduled {
                // Reload counter.
                gg.timers.counters[TIM] = gg[addr - 2];
            }

            // Need to start scheduling this timer
            let until_ov = Self::next_overflow_time(gg.timers.counters[TIM], new_ctrl);
            gg.timers.scheduled_at[TIM] = gg.scheduler.now() + 2;
            gg.scheduler
                .schedule(AdvEvent::TimerOverflow(TIM.u8()), until_ov);
        }

        gg[addr] = new_ctrl;
    }

    /// Handle an overflow and return time until next.
    fn overflow(gg: &mut GameGirlAdv, idx: u8) -> u32 {
        let addr = Self::hi_addr(idx);
        let reload = gg[addr - 2];
        let ctrl = gg[addr];
        // Set to reload value
        gg.timers.counters[idx.us()] = reload;
        // Fire IRQ if enabled
        if ctrl.is_bit(6) {
            Cpu::request_interrupt_idx(gg, Interrupt::Timer0 as u16 + idx.u16())
        }

        if idx < 2 {
            // Might need to notify APU about this
            let cnt = gg[SOUNDCNT_H];
            if cnt.bit(10).u8() == idx {
                Apu::timer_overflow::<0>(gg);
            }
            if cnt.bit(14).u8() == idx {
                Apu::timer_overflow::<1>(gg);
            }
        }

        if idx != 3 && gg[addr + 2].is_bit(2) {
            // Next timer is set to inc when we overflow.
            Self::inc_timer(gg, idx.us() + 1);
        }

        Self::next_overflow_time(reload, ctrl)
    }

    /// Time until next overflow, for scheduling.
    fn next_overflow_time(reload: u16, ctrl: u16) -> u32 {
        let scaler = DIVS[(ctrl & 3).us()].u32();
        (scaler * (0x1_0000 - reload.u32())) + 6
    }

    /// Increment a timer. Used for cascading timers.
    #[inline]
    fn inc_timer(gg: &mut GameGirlAdv, idx: usize) {
        let new = gg.timers.counters[idx].checked_add(1);
        match new {
            Some(val) => gg.timers.counters[idx] = val,
            None => {
                Self::overflow(gg, idx.u8());
            }
        }
    }

    /// Get the CTRL address of the given timer.
    #[inline]
    fn hi_addr(tim: u8) -> u32 {
        TM0CNT_H + (tim.u32() << 2)
    }
}
