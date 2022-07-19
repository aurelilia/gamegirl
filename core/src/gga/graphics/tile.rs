// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    gga::{
        addr::{BG0CNT, BG0HOFS, BG0VOFS, BG2PA, BG3PA, DISPCNT},
        graphics::{threading::PpuType, Ppu, BG0_EN},
    },
    numutil::{hword, word, NumExt},
};

impl Ppu {
    pub fn render_mode0(gg: &mut PpuType, line: u16) {
        Self::render_objs::<0>(gg, line);
        Self::render_bg_text::<0>(gg, line);
        Self::render_bg_text::<1>(gg, line);
        Self::render_bg_text::<2>(gg, line);
        Self::render_bg_text::<3>(gg, line);
    }

    pub fn render_mode1(gg: &mut PpuType, line: u16) {
        Self::render_objs::<0>(gg, line);
        Self::render_bg_text::<0>(gg, line);
        Self::render_bg_text::<1>(gg, line);
        Self::render_bg_affine::<2>(gg, BG2PA);
    }

    pub fn render_mode2(gg: &mut PpuType, line: u16) {
        Self::render_objs::<0>(gg, line);
        Self::render_bg_affine::<2>(gg, BG2PA);
        Self::render_bg_affine::<3>(gg, BG3PA);
    }

    fn render_bg_text<const IDX: u16>(gg: &mut PpuType, line: u16) {
        if !gg[DISPCNT].is_bit(BG0_EN + IDX) {
            return;
        }

        let offs = IDX.u32() * 2;
        let cnt = gg[BG0CNT + offs];
        let prio = cnt & 3;
        let mosaic = cnt.is_bit(6);
        let hofs = gg[BG0HOFS + offs * 2] as i16;
        let vofs = gg[BG0VOFS + offs * 2];
        let tile_base_addr = cnt.bits(2, 2).us() * 0x4000;
        let map_base = cnt.bits(8, 5).us() * 0x800;

        let size = cnt.bits(14, 2);
        let bpp8 = cnt.is_bit(7);
        let bg_y = line.wrapping_add(vofs);
        // TODO: Y-Mosaic

        for tile in -1..31 {
            let bg_x = (tile << 3) + hofs;
            let map_addr =
                map_base + Self::get_map_offset((bg_x >> 3) as u32, (bg_y >> 3).u32(), size).us();
            let map = hword(gg.ppu.vram[map_addr], gg.ppu.vram[map_addr + 1]);

            let tile_idx = map.bits(0, 10);
            let tile_y = if map.is_bit(11) {
                7 - (bg_y & 7)
            } else {
                bg_y & 7
            };
            let base_x = tile * 8 - (hofs & 0x7);
            let (x, x_step) = if map.is_bit(10) {
                (base_x + 7, -1)
            } else {
                (base_x, 1)
            };

            if bpp8 {
                let tile_addr = tile_base_addr + (tile_idx.us() * 64) + (tile_y.us() * 8);
                Self::render_tile_8bpp::<false>(gg, prio, x, x_step, tile_addr, mosaic, IDX.us());
            } else {
                let tile_addr = tile_base_addr + (tile_idx.us() * 32) + (tile_y.us() * 4);
                let palette = map.bits(12, 4).u8();
                Self::render_tile_4bpp::<false>(
                    gg,
                    prio,
                    x,
                    x_step,
                    tile_addr,
                    palette,
                    mosaic,
                    IDX.us(),
                );
            }
        }
    }

    fn render_bg_affine<const IDX: u16>(gg: &mut PpuType, offset: u32) {
        if !gg[DISPCNT].is_bit(BG0_EN + IDX) {
            return;
        }

        let offs = IDX.u32() * 2;
        let cnt = gg[BG0CNT + offs];
        let prio = cnt & 3;
        let mosaic = cnt.is_bit(6); // TODO
        let tile_base_addr = cnt.bits(2, 2).us() * 0x4000;
        let map_base = cnt.bits(8, 5).us() * 0x800;

        let size = cnt.bits(14, 2);
        let size = [128, 256, 512, 1024][size.us()];
        let overflow = cnt.is_bit(13);

        let bg_x = gg.ppu.bg_x[IDX.us() - 2];
        let bg_y = gg.ppu.bg_y[IDX.us() - 2];
        let pa = gg[offset] as i16 as i32;
        let pb = gg[offset + 2] as i16 as i32;
        let pc = gg[offset + 4] as i16 as i32;
        let pd = gg[offset + 6] as i16 as i32;

        for pixel_x in 0..240 {
            let mut x = (bg_x + pa * pixel_x) >> 8;
            let mut y = (bg_y + pc * pixel_x) >> 8;

            let range = 0..size;
            if !range.contains(&x) || !range.contains(&y) {
                if !overflow {
                    continue;
                } else {
                    x &= size - 1;
                    y &= size - 1;
                }
            }

            let map_addr = map_base + (((y >> 3) * (size >> 3)) + (x >> 3)) as usize;
            let map = gg.ppu.vram[map_addr];

            let tile_x = (x & 7) as usize;
            let tile_y = (y & 7) as usize;
            let tile_addr = tile_base_addr + (map.us() * 64) + (tile_y * 8) + tile_x;
            let colour = gg.ppu.vram[tile_addr];
            Self::set_pixel::<false>(gg, pixel_x as i16, prio, 0, colour, mosaic, IDX.us());
        }

        gg.ppu.bg_x[IDX.us() - 2] += pb;
        gg.ppu.bg_y[IDX.us() - 2] += pd;
    }

    pub(crate) fn get_affine_offs(lo: u16, hi: u16) -> i32 {
        if hi.is_bit(11) {
            (word(lo, hi & 0x7FF) | 0xF800_0000) as i32
        } else {
            word(lo, hi & 0x7FF) as i32
        }
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
}
