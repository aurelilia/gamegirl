// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::{components::scheduler::Kind, TimeS};
use NesEvent::*;

use crate::{apu::Apu, Nes};

/// All scheduler events on the NES.
#[derive(Copy, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum NesEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    #[default]
    PauseEmulation,
    ApuEvent(ApuEvent),
}

impl NesEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&self, nes: &mut Nes, late_by: TimeS) {
        match self {
            PauseEmulation => nes.ticking = false,
            ApuEvent(event) => Apu::handle_event(nes, *event, late_by),
        }
    }
}

impl Kind for NesEvent {}

/// Events the APU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum ApuEvent {
    // Push a sample to the output.
    PushSample,
}
