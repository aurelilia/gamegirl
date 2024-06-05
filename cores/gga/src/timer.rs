// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use arm_cpu::{Cpu, Interrupt};
use common::{numutil::NumExt, Time, TimeS};
use modular_bitfield::{bitfield, specifiers::*};

use crate::{audio::Apu, scheduling::AdvEvent, GameGirlAdv};

pub const DIVS: [u16; 4] = [1, 64, 256, 1024];

#[bitfield]
#[repr(u16)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimerCtrl {
    pub prescaler: B2,
    pub count_up: bool,
    #[skip]
    __: B3,
    pub irq_en: bool,
    pub enable: bool,
    #[skip]
    __: B8,
}

/// Timers on the GGA.
/// They run on the scheduler when in regular counting mode.
/// The scheduler variables have a bunch of small additions that work for some
/// reason, still not sure why. Some other timings that are inaccurate?
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Timers {
    // Registers
    pub reload: [u16; 4],
    pub control: [TimerCtrl; 4],

    /// Counter value. Used for cascading counters; for scheduled counters this
    /// will be the reload value (actual counter is calculated on read)
    counters: [u16; 4],
    /// The time the timer was scheduled, if it is on the scheduler.
    scheduled_at: [Time; 4],
}

impl Timers {
    /// Handle overflow of a scheduled timer.
    pub fn handle_overflow_event(gg: &mut GameGirlAdv, idx: u8, late_by: TimeS) {
        // Handle overflow
        let until_ov = Self::overflow(gg, idx) as TimeS;
        // Reschedule event
        // Edge case: with high reload and fast timers, sometimes (late_by > until_ov).
        // In this case, we simply schedule the next overflow event to be immediately.
        gg.timers.scheduled_at[idx.us()] = gg.scheduler.now() - late_by as Time + 2;
        gg.scheduler
            .schedule(AdvEvent::TimerOverflow(idx), until_ov - late_by);
    }

    /// Read current time of the given timer. Might be a bit expensive, since
    /// time needs to be calculated for timers on the scheduler.
    pub fn time_read<const TIM: usize>(gg: &GameGirlAdv) -> u16 {
        let ctrl = gg.timers.control[TIM];
        let is_scheduled = ctrl.enable() && (TIM == 0 || !ctrl.count_up());

        if is_scheduled {
            // Is on scheduler, calculate current value
            let scaler = DIVS[ctrl.prescaler().us()] as Time;
            let elapsed = gg.scheduler.now() - (gg.timers.scheduled_at[TIM] - 2);
            gg.timers.counters[TIM].wrapping_add((elapsed / scaler).u16())
        } else {
            // Either off or inc on overflow, just return current counter
            gg.timers.counters[TIM]
        }
    }

    /// Handle CTRL write by scheduling timer as appropriate.
    pub fn hi_write<const TIM: usize>(gg: &mut GameGirlAdv, new_ctrl: u16) {
        // Update current counter value first
        gg.timers.counters[TIM] = Self::time_read::<TIM>(gg);

        let old_ctrl = gg.timers.control[TIM];
        let new_ctrl: TimerCtrl = new_ctrl.into();
        let was_scheduled = old_ctrl.enable() && (TIM == 0 || !old_ctrl.count_up());
        let is_scheduled = new_ctrl.enable() && (TIM == 0 || !new_ctrl.count_up());

        if was_scheduled {
            // Need to cancel current scheduled event
            gg.scheduler
                .cancel_single(AdvEvent::TimerOverflow(TIM.u8()));
        }
        if is_scheduled {
            if !was_scheduled {
                // Reload counter.
                gg.timers.counters[TIM] = gg.timers.reload[TIM];
            }

            // Need to start scheduling this timer
            let until_ov = Self::next_overflow_time(gg.timers.counters[TIM], new_ctrl);
            gg.timers.scheduled_at[TIM] = gg.scheduler.now() + 2;
            gg.scheduler
                .schedule(AdvEvent::TimerOverflow(TIM.u8()), until_ov as TimeS);
        }

        gg.timers.control[TIM] = new_ctrl;
    }

    /// Handle an overflow and return time until next.
    fn overflow(gg: &mut GameGirlAdv, idx: u8) -> u32 {
        let ctrl = gg.timers.control[idx.us()];
        // Set to reload value
        gg.timers.counters[idx.us()] = gg.timers.reload[idx.us()];
        // Fire IRQ if enabled
        if ctrl.irq_en() {
            Cpu::request_interrupt_idx(gg, Interrupt::Timer0 as u16 + idx.u16());
        }

        if idx < 2 {
            // Might need to notify APU about this
            if gg.apu.cnt.a_timer() == idx {
                Apu::timer_overflow::<0>(gg);
            }
            if gg.apu.cnt.b_timer() == idx {
                Apu::timer_overflow::<1>(gg);
            }
        }

        if idx != 3 && gg.timers.control[idx.us() + 1].count_up() {
            // Next timer is set to inc when we overflow.
            Self::inc_timer(gg, idx.us() + 1);
        }

        Self::next_overflow_time(gg.timers.reload[idx.us()], ctrl)
    }

    /// Time until next overflow, for scheduling.
    fn next_overflow_time(reload: u16, ctrl: TimerCtrl) -> u32 {
        let scaler = DIVS[ctrl.prescaler().us()].u32();
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
}
