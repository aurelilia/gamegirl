// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use armchair::{Cpu, Interrupt};
use common::{components::scheduler::Scheduler, numutil::NumExt, Time, TimeS};
use modular_bitfield::{bitfield, specifiers::*};

use crate::{addr::TM0CNT_H, io::IoSection, scheduling::NdsEvent, NdsCpu};

/// All 2x to account for the ARM9's double clock speed,
/// which also affects the scheduler
const DIVS: [u16; 4] = [2, 128, 512, 2048];

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

/// Timers on the NDS. Separated by CPU.
/// They run on the scheduler when in regular counting mode.
/// The scheduler variables have a bunch of small additions that work for some
/// reason, still not sure why. Some other timings that are inaccurate?
///
/// Since they run on the scheduler, they are *all* timed by the
/// ARM9. Hopefully good enough?
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
    pub fn handle_overflow_event<DS: NdsCpu>(ds: &mut DS, idx: u8, late_by: TimeS) {
        // Handle overflow
        let until_ov = Self::overflow(ds, idx) as TimeS;
        // Reschedule event
        // Edge case: with high reload and fast timers, sometimes (late_by > until_ov).
        // In this case, we simply schedule the next overflow event to be immediately.
        ds.timers[DS::I].scheduled_at[idx.us()] = ds.scheduler.now() - late_by as Time + 2;
        ds.scheduler.schedule(
            NdsEvent::TimerOverflow {
                timer: idx,
                is_arm9: DS::I == 1,
            },
            until_ov - late_by,
        );
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
    pub fn hi_write(
        &mut self,
        is_arm9: bool,
        sched: &mut Scheduler<NdsEvent>,
        timer: usize,
        new_ctrl: IoSection<u16>,
    ) {
        let now = sched.now();
        // Update current counter value first
        self.counters[timer] = self.time_read(timer, now);

        let old_ctrl = self.control[timer];
        let new_ctrl = new_ctrl.apply_io_ret(&mut self.control[timer]);
        let was_scheduled = old_ctrl.enable() && (timer == 0 || !old_ctrl.count_up());
        let is_scheduled = new_ctrl.enable() && (timer == 0 || !new_ctrl.count_up());

        if was_scheduled {
            // Need to cancel current scheduled event
            sched.cancel_single(NdsEvent::TimerOverflow {
                timer: timer.u8(),
                is_arm9,
            });
        }
        if is_scheduled {
            if !was_scheduled {
                // Reload counter.
                self.counters[timer] = self.reload[timer];
            }

            // Need to start scheduling this timer
            let until_ov = Self::next_overflow_time(self.counters[timer], new_ctrl);
            self.scheduled_at[timer] = now + 2;
            sched.schedule(
                NdsEvent::TimerOverflow {
                    timer: timer.u8(),
                    is_arm9,
                },
                until_ov as TimeS,
            );
        }
    }

    /// Handle an overflow and return time until next.
    fn overflow<DS: NdsCpu>(ds: &mut DS, idx: u8) -> u32 {
        let ctrl = ds.timers[DS::I].control[idx.us()];
        let reload = ds.timers[DS::I].reload[idx.us()];
        // Set to reload value
        ds.timers[DS::I].counters[idx.us()] = reload;
        // Fire IRQ if enabled
        if ctrl.irq_en() {
            ds.cpu()
                .request_interrupt_with_index(Interrupt::Timer0 as u16 + idx.u16());
        }

        if idx != 3 && ds.timers[DS::I].control[idx.us() + 1].count_up() {
            // Next timer is set to inc when we overflow.
            Self::inc_timer(ds, idx.us() + 1);
        }

        Self::next_overflow_time(reload, ctrl)
    }

    /// Time until next overflow, for scheduling.
    fn next_overflow_time(reload: u16, ctrl: TimerCtrl) -> u32 {
        let scaler = DIVS[ctrl.prescaler().us()].u32();
        (scaler * (0x1_0000 - reload.u32())) + 6
    }

    /// Increment a timer. Used for cascading timers.
    #[inline]
    fn inc_timer<DS: NdsCpu>(ds: &mut DS, idx: usize) {
        let new = ds.timers[DS::I].counters[idx].checked_add(1);
        match new {
            Some(val) => ds.timers[DS::I].counters[idx] = val,
            None => {
                Self::overflow(ds, idx.u8());
            }
        }
    }
}
