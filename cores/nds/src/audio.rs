// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::TimeS;

use crate::{cpu::NDS9_CLOCK, scheduling::ApuEvent, Nds};

pub const SAMPLE_EVERY_N_CLOCKS: TimeS = (NDS9_CLOCK / 48000) as TimeS;

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Apu {}

impl Apu {
    /// Handle event. Since all APU events reschedule themselves, this
    /// function returns the time after which the event should repeat.
    pub fn handle_event(ds: &mut Nds, event: ApuEvent, late_by: TimeS) -> TimeS {
        match event {
            ApuEvent::PushSample => {
                ds.c.audio_buffer.push(0.0);
                ds.c.audio_buffer.push(0.0);
                SAMPLE_EVERY_N_CLOCKS - late_by
            }
        }
    }
}
