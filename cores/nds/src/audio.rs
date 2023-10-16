// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{cpu::NDS9_CLOCK, scheduling::ApuEvent, Nds};

pub const SAMPLE_EVERY_N_CLOCKS: i32 = (NDS9_CLOCK / 48000) as i32;

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Apu {
    pub buffer: Vec<f32>,
}

impl Apu {
    /// Handle event. Since all APU events reschedule themselves, this
    /// function returns the time after which the event should repeat.
    pub fn handle_event(ds: &mut Nds, event: ApuEvent, late_by: i32) -> i32 {
        match event {
            ApuEvent::PushSample => {
                ds.apu.buffer.push(0.0);
                ds.apu.buffer.push(0.0);
                SAMPLE_EVERY_N_CLOCKS - late_by
            }
        }
    }
}
