// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    scheduling::{ApuEvent, NesEvent},
    Nes, CLOCK_HZ,
};

const SAMPLE_EVERY_N_CLOCKS: i32 = CLOCK_HZ as i32 / 48000;

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Apu {
    pub buffer: Vec<f32>,
}

impl Apu {
    pub fn handle_event(nes: &mut Nes, event: ApuEvent, late_by: i32) {
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
