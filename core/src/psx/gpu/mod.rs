// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::Colour;

#[derive(Default, Deserialize, Serialize)]
pub struct Gpu {
    /// The last frame finished by the GPU, ready for display.
    #[serde(skip)]
    #[serde(default)]
    pub last_frame: Option<Vec<Colour>>,
}
