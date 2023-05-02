// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::components::scheduler::Kind;
use gga_ppu::{scheduling::PpuEvent, Ppu};
use psg_apu::GenApuEvent;
use AdvEvent::*;

use crate::{audio::Apu, timer::Timers, GameGirlAdv};

/// All scheduler events on the GGA.
#[derive(Copy, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum AdvEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    #[default]
    PauseEmulation,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
    /// An event handled by the APU.
    ApuEvent(ApuEvent),
    /// A timer overflow.
    TimerOverflow(u8),
}

impl AdvEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(self, gg: &mut GameGirlAdv, late_by: i32) {
        match self {
            PauseEmulation => gg.ticking = false,
            PpuEvent(evt) => Ppu::handle_event(gg, evt, late_by),
            ApuEvent(evt) => {
                let time = Apu::handle_event(gg, evt, late_by);
                gg.scheduler.schedule(self, time);
            }
            TimerOverflow(idx) => Timers::handle_overflow_event(gg, idx, late_by),
        }
    }
}

impl Kind for AdvEvent {}

/// Events the APU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum ApuEvent {
    /// Event from the generic CGB APU.
    Gen(GenApuEvent),
    /// Tick the CGB sequencer.
    Sequencer,
    /// Push a sample to the output.
    PushSample,
}
