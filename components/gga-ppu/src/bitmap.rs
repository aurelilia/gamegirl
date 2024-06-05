// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::NumExt;

use crate::{addr::*, threading::PpuType, Ppu, PpuSystem, BG2_EN, FRAME_SELECT};

impl<S: PpuSystem> Ppu<S>
where
    [(); S::W * S::H]:,
{
    pub fn render_mode3(gg: &mut PpuType<S>, line: u16) {
        if !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let line_start = line.us() * S::W;
        for offs in 0..S::W {
            let pixel = line_start + offs;
            gg.ppu.bg_layers[0][offs] = gg.ppu.hword_to_colour_vram(pixel << 1);
        }

        Self::render_objs(gg, line);
    }

    pub fn render_mode4(gg: &mut PpuType<S>, line: u16) {
        if !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let line_start = line.us() * S::W;
        let start_addr = Self::bitmap_start_addr(gg) + line_start;
        for offs in 0..S::W {
            let palette = gg.ppu.vram[start_addr + offs];
            if palette != 0 {
                gg.ppu.bg_layers[0][offs] = gg.ppu.idx_to_palette::<false>(palette);
            }
        }

        Self::render_objs(gg, line);
    }

    pub fn render_mode5(gg: &mut PpuType<S>, line: u16) {
        if line > 127 || !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let vram_start = Self::bitmap_start_addr(gg);
        let line_start = vram_start + (line.us() * 160);
        for offs in 0..160 {
            let pixel = (line_start + offs).us();
            gg.ppu.bg_layers[0][offs] = gg.ppu.hword_to_colour_vram(pixel << 1);
        }

        Self::render_objs(gg, line);
    }

    fn bitmap_start_addr(gg: &PpuType<S>) -> usize {
        if gg[DISPCNT].is_bit(FRAME_SELECT) {
            0xA000
        } else {
            0x0
        }
    }
}
