// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod gga;

use common::Colour;
use gga_ppu::threading::{new_ppu, GgaPpu};

use crate::{Nds7, Nds9};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct NdsEngines {
    /// See notes in `gga.rs`
    pub(crate) ppu_a: GgaPpu<Nds7>,
    /// See notes in `gga.rs`
    pub(crate) ppu_b: GgaPpu<Nds9>,
    pub last_frame: Option<Vec<Colour>>,
}

impl Default for NdsEngines {
    fn default() -> Self {
        Self {
            ppu_a: new_ppu(),
            ppu_b: new_ppu(),
            last_frame: None,
        }
    }
}
