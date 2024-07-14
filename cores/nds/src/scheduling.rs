// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::{components::scheduler::Kind, TimeS};
use NdsEvent::*;

use crate::{audio::Apu, timer::Timers, Nds};

// TODO
type PpuEvent = ();

#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum NdsEvent {
    PauseEmulation,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
    /// An event handled by the APU.
    ApuEvent(ApuEvent),
    /// A timer overflow.
    TimerOverflow {
        timer: u8,
        is_arm9: bool,
    },
}

impl NdsEvent {
    pub fn dispatch(self, ds: &mut Nds, late_by: TimeS) {
        match self {
            PauseEmulation => ds.c.in_tick = false,
            PpuEvent(evt) => {
                // Ppu::handle_event(&mut ds.nds7(), evt, late_by);
                // Ppu::handle_event(&mut ds.nds9(), evt, late_by);
            }
            ApuEvent(evt) => {
                let time = Apu::handle_event(ds, evt, late_by);
                ds.scheduler.schedule(self, time);
            }
            TimerOverflow { timer, is_arm9 } if is_arm9 => {
                Timers::handle_overflow_event(&mut ds.nds9(), timer, late_by)
            }
            TimerOverflow { timer, .. } => {
                Timers::handle_overflow_event(&mut ds.nds7(), timer, late_by)
            }
        }
    }
}

impl Kind for NdsEvent {}

impl Default for NdsEvent {
    fn default() -> Self {
        Self::PauseEmulation
    }
}

/// Events the APU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum ApuEvent {
    /// Push a sample to the output.
    PushSample,
}
