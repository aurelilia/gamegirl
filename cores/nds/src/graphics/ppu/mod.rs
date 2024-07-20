// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! For this PPU implementation, I took a lot of reference from DenSinH's GBAC-.
//! It is not an outright copy, but I want to thank them for their code
//! that helped me understand the PPU's more complex behavior.
//! The code is under the MIT license at https://github.com/DenSinH/GBAC-.
//! Additionally RustBoyAdvance-ng by michelhe was heavily used for the
//! second attempt at an implementation. Thank you to michelhe, too.
//! The code is under the MIT license at  https://github.com/michelhe/rustboyadvance-ng.
//!
//! This PPU implementation is a hard fork of the PPU of the GGA core.
//! This decision was made due to the DS diverging enough that trying to keep
//! the code generic would be more trouble than it's worth.
//!
//! TODO: Missing features:
//! - Main Memory Display, Video Capture
//! - Extended Palettes
//! - Extended BG modes
//! - Bitmap transparency
//! - OBJ priority changes
//! - OBJ tile mappings not on the GBA
//! - Bitmap OBJs

pub mod registers;
mod render;

use std::sync::Arc;

use arm_cpu::{Cpu, Interrupt};
use common::{common::video::FrameBuffer, numutil::NumExt, Colour, UnsafeArc};
use registers::*;
use render::{PpuRender, PpuRendererKind};

use crate::{
    hw::dma::{Dmas, Reason},
    memory::KB,
    scheduling::{NdsEvent, PpuEvent},
    Nds, Nds9,
};

pub(super) const WIDTH: usize = 256;
pub(super) const HEIGHT: usize = 192;
pub(super) const VBLANK_END: u16 = 263;
const BLACK: Colour = [0, 0, 0, 255];
const WHITE: Colour = [31, 31, 31, 255];
const TRANS: Colour = [0, 0, 0, 0];

#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Point(i32, i32);

impl Point {
    fn inbounds(self, w: usize, h: usize) -> bool {
        let Point(x, y) = self;
        x >= 0 && x < w as i32 && y >= 0 && y < h as i32
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Ppu {
    // Registers
    pub regs: PpuRegisters,

    // Memory
    pub palette: Arc<[u8]>,
    pub oam: Arc<[u8]>,

    // Renderer
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(skip))]
    render: PpuRendererKind,
}

impl Ppu {
    pub(super) fn render_line(&mut self, vcount: u16) {
        self.regs.vcount = vcount;
        self.render.do_line(self.regs.clone());

        // Update affines
        for bg in 2..4 {
            if self.regs.bg_enabled(bg.u16()) {
                self.regs.bg_scale[bg - 2].latched.0 += self.regs.bg_scale[bg - 2].pb as i32;
                self.regs.bg_scale[bg - 2].latched.1 += self.regs.bg_scale[bg - 2].pd as i32;
            }
        }
    }

    pub(super) fn end_frame(&mut self) {
        // Reload affine backgrounds
        self.regs.bg_scale[0].latch();
        self.regs.bg_scale[1].latch();
    }

    pub(super) fn get_output(&mut self) -> Option<Vec<[u8; 4]>> {
        self.render.get_last()
    }

    pub fn init_render(ds: &mut Nds) {
        for ppu in 0..2 {
            let render = PpuRender::new(
                Arc::clone(&ds.gpu.ppus[ppu].palette),
                UnsafeArc::clone(&ds.gpu.vram),
                Arc::clone(&ds.gpu.ppus[ppu].oam),
            );
            ds.gpu.ppus[ppu].render = PpuRendererKind::new(render, ds.c.config.threaded_ppu);
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            regs: Default::default(),

            palette: Arc::new([0; KB]),
            oam: Arc::new([0; KB]),

            render: PpuRendererKind::Invalid,
        }
    }
}
