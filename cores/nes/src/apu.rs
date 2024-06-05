// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::TimeS;

use crate::{
    scheduling::{ApuEvent, NesEvent},
    Nes, CLOCK_HZ,
};

const SAMPLE_EVERY_N_CLOCKS: TimeS = CLOCK_HZ as TimeS / 48000;

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Apu {
    pub buffer: Vec<f32>,
}

impl Apu {
    pub fn handle_event(nes: &mut Nes, event: ApuEvent, late_by: TimeS) {
        match event {
            ApuEvent::PushSample => {
                nes.apu.buffer.push(0.0);
                nes.apu.buffer.push(0.0);
                nes.scheduler.schedule(
                    NesEvent::ApuEvent(ApuEvent::PushSample),
                    SAMPLE_EVERY_N_CLOCKS - late_by,
                )
            }
        }
    }
}
