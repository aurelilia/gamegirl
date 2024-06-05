// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use arm_cpu::{Cpu, Interrupt};
use common::{numutil::NumExt, Time, TimeS};

use crate::{addr::TM0CNT_H, scheduling::NdsEvent, NdsCpu};

/// All 2x to account for the ARM9's double clock speed,
/// which also affects the scheduler
const DIVS: [u16; 4] = [2, 128, 512, 2048];

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
    pub fn time_read<DS: NdsCpu, const TIM: usize>(ds: &DS) -> u16 {
        let ctrl = ds[Self::hi_addr(TIM.u8())];
        let is_scheduled = ctrl.is_bit(7) && (TIM == 0 || !ctrl.is_bit(2));

        if is_scheduled {
            // Is on scheduler, calculate current value
            let scaler = DIVS[(ctrl & 3).us()] as Time;
            let elapsed = ds.scheduler.now() - ds.timers[DS::I].scheduled_at[TIM];
            ds.timers[DS::I].counters[TIM].wrapping_add((elapsed / scaler).u16())
        } else {
            // Either off or inc on overflow, just return current counter
            ds.timers[DS::I].counters[TIM]
        }
    }

    /// Handle CTRL write by scheduling timer as appropriate.
    pub fn hi_write<DS: NdsCpu, const TIM: usize>(ds: &mut DS, addr: u32, new_ctrl: u16) {
        // Update current counter value first
        ds.timers[DS::I].counters[TIM] = Self::time_read::<DS, TIM>(ds);

        let old_ctrl = ds[addr];
        let was_scheduled = old_ctrl.is_bit(7) && (TIM == 0 || !old_ctrl.is_bit(2));
        let is_scheduled = new_ctrl.is_bit(7) && (TIM == 0 || !new_ctrl.is_bit(2));

        if was_scheduled {
            // Need to cancel current scheduled event
            ds.scheduler.cancel_single(NdsEvent::TimerOverflow {
                timer: TIM.u8(),
                is_arm9: DS::I == 1,
            });
        }
        if is_scheduled {
            if !was_scheduled {
                // Reload counter.
                ds.timers[DS::I].counters[TIM] = ds[addr - 2];
            }

            // Need to start scheduling this timer
            let until_ov = Self::next_overflow_time(ds.timers[DS::I].counters[TIM], new_ctrl);
            ds.timers[DS::I].scheduled_at[TIM] = ds.scheduler.now() + 2;
            ds.scheduler.schedule(
                NdsEvent::TimerOverflow {
                    timer: TIM.u8(),
                    is_arm9: DS::I == 1,
                },
                until_ov as TimeS,
            );
        }

        ds[addr] = new_ctrl;
    }

    /// Handle an overflow and return time until next.
    fn overflow<DS: NdsCpu>(ds: &mut DS, idx: u8) -> u32 {
        let addr = Self::hi_addr(idx);
        let reload = ds[addr - 2];
        let ctrl = ds[addr];
        // Set to reload value
        ds.timers[DS::I].counters[idx.us()] = reload;
        // Fire IRQ if enabled
        if ctrl.is_bit(6) {
            Cpu::request_interrupt_idx(ds, Interrupt::Timer0 as u16 + idx.u16());
        }

        if idx != 3 && ds[addr + 2].is_bit(2) {
            // Next timer is set to inc when we overflow.
            Self::inc_timer(ds, idx.us() + 1);
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
    fn inc_timer<DS: NdsCpu>(ds: &mut DS, idx: usize) {
        let new = ds.timers[DS::I].counters[idx].checked_add(1);
        match new {
            Some(val) => ds.timers[DS::I].counters[idx] = val,
            None => {
                Self::overflow(ds, idx.u8());
            }
        }
    }

    /// Get the CTRL address of the given timer.
    #[inline]
    fn hi_addr(tim: u8) -> u32 {
        TM0CNT_H + (tim.u32() << 2)
    }
}
