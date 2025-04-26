// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::{components::scheduler::Kind, TimeS};
use AdvEvent::*;

use crate::{
    audio::{psg::GenApuEvent, Apu},
    cpu::GgaFullBus,
    hw::timer::Timers,
    ppu::Ppu,
};

/// All scheduler events on the GGA.
#[derive(Copy, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum AdvEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    #[default]
    PauseEmulation,
    /// Update button inputs.
    UpdateKeypad,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
    /// An event handled by the APU.
    ApuEvent(ApuEvent),
    /// A timer overflow.
    TimerOverflow(u8),
}

impl GgaFullBus<'_> {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&mut self, event: AdvEvent, late_by: TimeS) {
        match event {
            PauseEmulation => self.bus.c.in_tick = false,
            UpdateKeypad => self.check_keycnt(),
            PpuEvent(evt) => Ppu::handle_event(self, evt, late_by),
            ApuEvent(evt) => {
                let time = Apu::handle_event(self, evt, late_by);
                self.scheduler.schedule(event, time);
            }
            TimerOverflow(idx) => Timers::handle_overflow_event(self, idx, late_by),
        }
    }
}

impl Kind for AdvEvent {}

/// Events the APU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ApuEvent {
    /// Event from the generic CGB APU.
    Gen(GenApuEvent),
    /// Tick the CGB sequencer.
    Sequencer,
    /// Push a sample to the output.
    PushSample,
}

/// Events the PPU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PpuEvent {
    /// Start of HBlank.
    HblankStart,
    /// Set HBlank flag in DISPSTAT (this is delayed by 46 cycles)
    SetHblank,
    /// End of HBlank, which is the start of the next scanline.
    HblankEnd,
}
