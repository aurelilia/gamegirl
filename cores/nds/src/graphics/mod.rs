// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::Colour;
use gga_ppu::threading::{new_ppu, GgaPpu};

use crate::{Nds7, Nds9};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct NdsEngines {
    pub last_frame: Option<Vec<Colour>>,
}

impl Default for NdsEngines {
    fn default() -> Self {
        Self { last_frame: None }
    }
}
