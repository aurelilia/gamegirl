// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::{
    gga::graphics::threading::{new_ppu, GgaPpu},
    Colour,
};

#[derive(Deserialize, Serialize)]
pub struct NdsEngines {
    pub ppus: [GgaPpu; 2],
    pub last_frame: Option<Vec<Colour>>,
}

impl Default for NdsEngines {
    fn default() -> Self {
        Self {
            ppus: [new_ppu(), new_ppu()],
            last_frame: None,
        }
    }
}
