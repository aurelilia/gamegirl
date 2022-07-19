// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    gga::{
        addr::*,
        graphics::{threading::PpuType, Ppu, BG2_EN, FRAME_SELECT},
    },
    numutil::NumExt,
};

impl Ppu {
    pub fn render_mode3(gg: &mut PpuType, line: u16) {
        if !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let line_start = line.us() * 240;
        for offs in 0..240 {
            let pixel = line_start + offs;
            gg.ppu.bg_layers[0][offs] = gg.ppu.hword_to_colour_vram(pixel << 1);
        }

        Self::render_objs::<512>(gg, line);
    }

    pub fn render_mode4(gg: &mut PpuType, line: u16) {
        if !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let line_start = line.us() * 240;
        let start_addr = Self::bitmap_start_addr(gg) + line_start;
        for offs in 0..240 {
            let palette = gg.ppu.vram[start_addr + offs];
            if palette != 0 {
                gg.ppu.bg_layers[0][offs] = gg.ppu.idx_to_palette::<false>(palette);
            }
        }

        Self::render_objs::<512>(gg, line);
    }

    pub fn render_mode5(gg: &mut PpuType, line: u16) {
        if line > 127 || !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let vram_start = Self::bitmap_start_addr(gg);
        let line_start = vram_start + (line.us() * 160);
        for offs in 0..160 {
            let pixel = (line_start + offs).us();
            gg.ppu.bg_layers[0][offs] = gg.ppu.hword_to_colour_vram(pixel << 1);
        }

        Self::render_objs::<512>(gg, line);
    }

    fn bitmap_start_addr(gg: &PpuType) -> usize {
        if !gg[DISPCNT].is_bit(FRAME_SELECT) {
            0x0
        } else {
            0xA000
        }
    }
}
