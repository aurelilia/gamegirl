// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::{components::scheduler::Kind, nds::Nds, psx::PlayStation};

#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum NdsEvent {
    PauseEmulation,
}

impl NdsEvent {
    pub fn dispatch(self, _ds: &mut Nds, _late_by: i32) {}
}

impl Kind for NdsEvent {}

impl Default for NdsEvent {
    fn default() -> Self {
        Self::PauseEmulation
    }
}
