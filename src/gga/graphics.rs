use super::memory::KB;
use crate::Colour;
use serde::{Deserialize, Serialize};

pub const PPU_REG_START: usize = 0x04000000;
pub const PPU_REG_END: usize = 0x04000055;
pub const PALETTE_START: usize = 0x05000000;
pub const PALETTE_END: usize = 0x050003FF;
pub const VRAM_START: usize = 0x06000000;
pub const VRAM_END: usize = 0x06017FFF;
pub const OAM_START: usize = 0x07000000;
pub const OAM_END: usize = 0x070003FF;

#[derive(Deserialize, Serialize)]
pub struct Ppu {
    #[serde(with = "serde_arrays")]
    pub regs: [u8; 0x56],
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
