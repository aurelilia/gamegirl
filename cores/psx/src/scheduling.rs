// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{components::scheduler::Kind, TimeS};

use crate::{PlayStation, FRAME_CLOCK, SAMPLE_CLOCK};

#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PsxEvent {
    PauseEmulation,
    OutputFrame,
    ProduceSample,
}

impl PsxEvent {
    pub fn dispatch(self, ps: &mut PlayStation, _late_by: TimeS) {
        match self {
            PsxEvent::PauseEmulation => ps.ticking = false,
            PsxEvent::OutputFrame => {
                ps.ppu.output_frame();
                ps.scheduler.schedule(PsxEvent::OutputFrame, FRAME_CLOCK);
            }
            PsxEvent::ProduceSample => {
                ps.apu.buffer.push(0.0);
                ps.apu.buffer.push(0.0);
                ps.scheduler.schedule(PsxEvent::ProduceSample, SAMPLE_CLOCK);
            }
        }
    }
}

impl Kind for PsxEvent {}

impl Default for PsxEvent {
    fn default() -> Self {
        Self::PauseEmulation
    }
}
