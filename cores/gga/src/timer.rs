// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use arm_cpu::{Cpu, Interrupt};
use common::{components::scheduler::Scheduler, numutil::NumExt, Time, TimeS};
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
        Self::overflow(gg, idx, -late_by);
    }

    /// Read current time of the given timer. Might be a bit expensive, since
    /// time needs to be calculated for timers on the scheduler.
    pub fn time_read(&self, timer: usize, now: Time) -> u16 {
        let ctrl = self.control[timer];
        let is_scheduled = ctrl.enable() && (timer == 0 || !ctrl.count_up());

        if is_scheduled {
            // Is on scheduler, calculate current value
            let scaler = DIVS[ctrl.prescaler().us()] as Time;
            let elapsed = now - self.scheduled_at[timer];
            self.counters[timer].wrapping_add((elapsed / scaler).u16())
        } else {
            // Either off or inc on overflow, just return current counter
            self.counters[timer]
        }
    }

    /// Handle CTRL write by scheduling timer as appropriate.
    pub fn hi_write(&mut self, sched: &mut Scheduler<AdvEvent>, timer: usize, new_ctrl: u16) {
        let now = sched.now();
        // Update current counter value first
        self.counters[timer] = self.time_read(timer, now);

        let old_ctrl = self.control[timer];
        let new_ctrl: TimerCtrl = new_ctrl.into();
        let was_scheduled = old_ctrl.enable() && (timer == 0 || !old_ctrl.count_up());
        let is_scheduled = new_ctrl.enable() && (timer == 0 || !new_ctrl.count_up());

        if was_scheduled {
            // Need to cancel current scheduled event
            sched.cancel_single(AdvEvent::TimerOverflow(timer.u8()));
        }
        if is_scheduled {
            if !was_scheduled {
                // Reload counter.
                self.counters[timer] = self.reload[timer];
            }
            self.start_timer(sched, timer, new_ctrl, 2, 0);
        }

        self.control[timer] = new_ctrl;
    }

    fn start_timer(
        &mut self,
        sched: &mut Scheduler<AdvEvent>,
        timer: usize,
        new_ctrl: TimerCtrl,
        mut base_offset: TimeS,
        mut timer_offset: TimeS,
    ) {
        let (until_ov, mask) = Self::overflow_time(self.counters[timer], new_ctrl);
        if until_ov == 1 {
            // Bit of a hack.
            timer_offset += 1;
        }
        self.scheduled_at[timer] = sched
            .now()
            .wrapping_add_signed(base_offset)
            .wrapping_add_signed(timer_offset);
        sched.schedule(
            AdvEvent::TimerOverflow(timer.u8()),
            until_ov as TimeS + base_offset + 3,
        );
    }

    /// Handle an overflow and return time until next.
    fn overflow(gg: &mut GameGirlAdv, idx: u8, offset: TimeS) {
        let ctrl = gg.timers.control[idx.us()];
        let reload = gg.timers.reload[idx.us()];
        // Set to reload value
        gg.timers.counters[idx.us()] = reload;
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

        if !ctrl.count_up() {
            Self::start_timer(
                &mut gg.timers,
                &mut gg.scheduler,
                idx.us(),
                ctrl,
                offset,
                -7,
            )
        }
    }

    /// Time until an overflow, for scheduling.
    /// Second return is the prescaler mask.
    fn overflow_time(reload: u16, ctrl: TimerCtrl) -> (u32, u32) {
        let scaler = DIVS[ctrl.prescaler().us()].u32();
        ((scaler * (0x1_0000 - reload.u32())), (scaler - 1))
    }

    /// Increment a timer. Used for cascading timers.
    #[inline]
    fn inc_timer(gg: &mut GameGirlAdv, idx: usize) {
        let new = gg.timers.counters[idx].checked_add(1);
        match new {
            Some(val) => gg.timers.counters[idx] = val,
            None => {
                Self::overflow(gg, idx.u8(), 0);
            }
        }
    }
}
