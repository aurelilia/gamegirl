use crate::{
    gga::{
        addr::{BG0CNT, BG0HOFS, BG0VOFS, DISPCNT},
        graphics::{Ppu, BG0_EN, WIN0_EN, WIN1_EN},
        GameGirlAdv,
    },
    numutil::{hword, NumExt},
};

impl Ppu {
    pub fn render_mode0(gg: &mut GameGirlAdv, line: u16) {
        Self::render_bg_text::<0>(gg, line);
        Self::render_bg_text::<1>(gg, line);
        Self::render_bg_text::<2>(gg, line);
        Self::render_bg_text::<3>(gg, line);
        Self::render_objs::<0>(gg, line);
    }

    fn render_bg_text<const IDX: u16>(gg: &mut GameGirlAdv, line: u16) {
        if !gg[DISPCNT].is_bit(BG0_EN + IDX) {
            return;
        }

        let offs = (IDX.u32() * 2);
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

        for tile in -1..30 {
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
                Self::render_tile_8bpp::<false>(gg, prio, x, x_step, tile_addr, mosaic);
            } else {
                let tile_addr = tile_base_addr + (tile_idx.us() * 32) + (tile_y.us() * 4);
                let palette = map.bits(12, 4).u8() << 4;
                Self::render_tile_4bpp::<false>(gg, prio, x, x_step, tile_addr, palette, mosaic);
            }
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
