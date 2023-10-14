// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::iter;

use common::Colour;
use modular_bitfield::{
    bitfield,
    specifiers::{B1, B2, B4},
    BitfieldSpecifier,
};

use crate::PlayStation;

#[bitfield]
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct GpuStat {
    texture_x_base: B4,
    texture_y_base: B1,
    semi_transparency: B2,
    texture_depth: TextureDepth,
    dither_enable: bool,
    draw_to_display: bool,
    mask_pixels: bool,
    draw_pixels: bool,
    interlace_field: bool,
    reverse_flag: bool,
    disable_textures: bool,
    horizontal_force_368: bool,
    horizontal_res: HorizontalRes,
    vertical_is_480: bool,
    is_pal: bool,
    colour_depth_24: bool,
    vertical_interlace: bool,
    disp_enable: bool,
    intr_req: bool,
    dma_req: bool,
    cmd_ready: bool,
    vram_ready: bool,
    dma_ready: bool,
    dma_dir: DmaDirection,
    interlace_is_odd: bool,
}

impl Default for GpuStat {
    fn default() -> Self {
        Self::from(0x1C00_0000)
    }
}

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug)]
pub enum TextureDepth {
    Bit4 = 0,
    Bit8 = 1,
    Bit15 = 2,
    Reserved = 3,
}

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug)]
pub enum HorizontalRes {
    H256 = 0,
    H320 = 1,
    H512 = 2,
    H640 = 3,
}

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug)]
pub enum DmaDirection {
    Off = 0,
    Reserved = 1,
    CpuToGp0 = 2,
    GpuToCpu = 3,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Gpu {
    pub stat: GpuStat,

    /// The last frame finished by the GPU, ready for display.
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_frame: Option<Vec<Colour>>,
}

impl Gpu {
    pub fn process_command(ps: &mut PlayStation, cmd: u32) {
        log::debug!("GPU command: 0x{cmd:08X}");
    }

    pub fn output_frame(&mut self) {
        let vec = iter::repeat([0; 4]).take(640 * 480).collect();
        self.last_frame = Some(vec);
    }
}
