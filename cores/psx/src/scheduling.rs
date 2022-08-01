// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::components::scheduler::Kind;
use serde::{Deserialize, Serialize};

use crate::PlayStation;

#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum PsxEvent {
    PauseEmulation,
}

impl PsxEvent {
    pub fn dispatch(self, _ps: &mut PlayStation, _late_by: i32) {}
}

impl Kind for PsxEvent {}

impl Default for PsxEvent {
    fn default() -> Self {
        Self::PauseEmulation
    }
}
