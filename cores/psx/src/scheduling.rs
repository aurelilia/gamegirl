// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

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
