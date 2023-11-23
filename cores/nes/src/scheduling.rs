// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::components::scheduler::Kind;
use NesEvent::*;

use crate::Nes;

/// All scheduler events on the NES.
#[derive(Copy, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum NesEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    #[default]
    PauseEmulation,
}

impl NesEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&self, nes: &mut Nes, late_by: i32) {
        match self {
            PauseEmulation => nes.ticking = false,
        }
    }
}

impl Kind for NesEvent {}
