use super::memory::KB;
use crate::Colour;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Ppu {
    #[serde(with = "serde_arrays")]
    pub palette: [u8; KB],
    #[serde(with = "serde_arrays")]
    pub vram: [u8; 96 * KB],
    #[serde(with = "serde_arrays")]
    pub oam: [u8; KB],

    #[serde(skip)]
    #[serde(default)]
    pub last_frame: Option<Vec<Colour>>,
}
