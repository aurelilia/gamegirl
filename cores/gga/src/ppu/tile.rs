// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::numutil::{hword, NumExt};

use super::{OverflowMode, PaletteMode, Point, Ppu, WIDTH};

impl Ppu {
    pub(super) fn render_bg_text(&mut self, bg: u16) {
        if !self.bg_enabled(bg) {
            return;
        }

        let (hofs, vofs) = (
            self.bg_offsets[bg.us() * 2] as i16,
            self.bg_offsets[(bg.us() * 2) + 1],
        );
        let cnt = self.bg_cnt[bg.us()];
        let screen_block_base = cnt.screen_base_block().us() * 0x800;
        let char_block_base = cnt.character_base_block().us() * 0x4000;
        let size = cnt.screen_size();
        let bg_y = self.vcount.wrapping_add(vofs);

        for tile in -1..31 {
            let bg_x = (tile << 3) + hofs;
            let map_addr = screen_block_base
                + Self::get_map_offset((bg_x >> 3) as u32, (bg_y >> 3).u32(), size.u16()).us();
            let map = hword(self.vram[map_addr], self.vram[map_addr + 1]);

            let tile_idx = map.bits(0, 10);
            let tile_y = if map.is_bit(11) {
                7 - (bg_y & 7)
            } else {
                bg_y & 7
            };
            let base_x = tile * 8 - (hofs & 0x7);
            let (mut x, x_step) = if map.is_bit(10) {
                (base_x + 7, -1)
            } else {
                (base_x, 1)
            };

            if cnt.palette_mode() == PaletteMode::Single256 {
                let tile_addr = char_block_base + (tile_idx.us() * 64) + (tile_y.us() * 8);
                for idx in 0..8 {
                    let colour = self.vram[tile_addr + idx];
                    self.set_pixel(bg, x, 0, colour);
                    x += x_step;
                }
            } else {
                let tile_addr = char_block_base + (tile_idx.us() * 32) + (tile_y.us() * 4);
                let palette = map.bits(12, 4).u8();
                for idx in 0..4 {
                    let byte = self.vram[tile_addr + idx];
                    self.set_pixel(bg, x, palette, byte & 0xF);
                    x += x_step;
                    self.set_pixel(bg, x, palette, byte >> 4);
                    x += x_step;
                }
            }
        }
    }

    pub(super) fn set_pixel(&mut self, bg: u16, x: i16, palette: u8, colour_idx: u8) {
        if !(0..(WIDTH as i16)).contains(&x) || colour_idx == 0 {
            return;
        }
        let colour = self.idx_to_palette::<false>((palette << 4) + colour_idx);
        self.bg_layers[bg.us()][x as usize] = colour;
    }

    // Adapted from https://github.com/DenSinH/GBAC-/blob/f460ad61fcd4c90429f47435d49b23310185f916/GBAEmulator/PPU/PPU.Render.BG.cs#L49
    // Thank you to DenSinH!
    fn get_map_offset(x: u32, y: u32, size: u16) -> u32 {
        match size {
            0 => ((y & 0x1f) << 6) | ((x & 0x1f) << 1),
            1 => (if (x & 0x3f) > 31 { 0x800 } else { 0 }) | ((y & 0x1f) << 6) | ((x & 0x1f) << 1),
            2 => (if (y & 0x3f) > 31 { 0x800 } else { 0 }) | ((y & 0x1f) << 6) | ((x & 0x1f) << 1),
            _ => {
                (if (y & 0x3f) > 31 { 0x1000 } else { 0 })
                    | (if (x & 0x3f) > 31 { 0x800 } else { 0 })
                    | ((y & 0x1f) << 6)
                    | ((x & 0x1f) << 1)
            }
        }
    }
    pub(super) fn render_bg_affine(&mut self, bg: u16) {
        if !self.bg_enabled(bg) {
            return;
        }

        let cnt = self.bg_cnt[bg.us()];
        let screen_block_base = cnt.screen_base_block().us() * 0x800;
        let char_block_base = cnt.character_base_block().us() * 0x4000;
        let size = [128, 256, 512, 1024][cnt.screen_size().us()];
        let scal = self.bg_scale[bg.us() - 2];

        let Point(bg_x, bg_y) = scal.latched;
        for pixel_x in 0..(WIDTH as i32) {
            let mut x = (bg_x + scal.pa as i32 * pixel_x) >> 8;
            let mut y = (bg_y + scal.pc as i32 * pixel_x) >> 8;

            let range = 0..size;
            if !range.contains(&x) || !range.contains(&y) {
                if cnt.overflow_mode() != OverflowMode::Wraparound {
                    continue;
                }
                x &= size - 1;
                y &= size - 1;
            }

            let map_addr = screen_block_base + (((y >> 3) * (size >> 3)) + (x >> 3)) as usize;
            let map = self.vram[map_addr];

            let tile_x = (x & 7) as usize;
            let tile_y = (y & 7) as usize;
            let tile_addr = char_block_base + (map.us() * 64) + (tile_y * 8) + tile_x;
            let colour = self.vram[tile_addr];
            self.set_pixel(bg, pixel_x as i16, 0, colour);
        }
    }
}