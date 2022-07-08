use serde::{Deserialize, Serialize};

use crate::Colour;

#[derive(Default, Deserialize, Serialize)]
pub struct Gpu {
    /// The last frame finished by the GPU, ready for display.
    #[serde(skip)]
    #[serde(default)]
    pub last_frame: Option<Vec<Colour>>,
}
