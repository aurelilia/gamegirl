// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    ggc::{
        io::{
            addr::BGP,
            ppu::{cgb::Cgb, Ppu, PpuKind},
        },
        GameGirl,
    },
    numutil::NumExt,
    Colour,
};

pub const COLOURS: [u8; 4] = [255, 191, 63, 0];

impl Ppu {
    pub fn dmg_render_bg_or_window(
        gg: &mut GameGirl,
        scroll_x: u8,
        start_x: u8,
        end_x: u8,
        map_addr: u16,
        map_line: u8,
        correct_tile_addr: bool,
    ) {
        let colours = Self::get_bg_colours(gg);
        let line = gg.ppu.line;
        let mut tile_x = scroll_x & 7;
        let tile_y = map_line & 7;
        let mut tile_addr = map_addr + ((map_line / 8).u16() * 0x20) + (scroll_x >> 3).u16();
        let mut tile_data_addr =
            Self::bg_tile_data_addr(gg, gg.mem.vram[tile_addr.us()]) + (tile_y.u16() * 2);
        let mut high = gg.mem.vram[tile_data_addr.us() + 1];
        let mut low = gg.mem.vram[tile_data_addr.us()];

        for tile_idx_addr in start_x..end_x {
            let colour_idx = (high.bit(7 - tile_x.u16()) << 1) + low.bit(7 - tile_x.u16());
            gg.ppu().bg_occupied_pixels[tile_idx_addr.us()] |= colour_idx != 0;
            gg.ppu()
                .set_pixel(tile_idx_addr, line, colours[colour_idx.us()]);

            tile_x += 1;
            if tile_x == 8 {
                tile_x = 0;
                tile_addr = if correct_tile_addr && (tile_addr & 0x1F) == 0x1F {
                    tile_addr - 0x1F
                } else {
                    tile_addr + 1
                };
                tile_data_addr =
                    Self::bg_tile_data_addr(gg, gg.mem.vram[tile_addr.us()]) + (tile_y.u16() * 2);
                high = gg.mem.vram[tile_data_addr.us() + 1];
                low = gg.mem.vram[tile_data_addr.us()];
            }
        }
    }

    pub fn clear_line(gg: &mut GameGirl) {
        let y = gg.ppu.line;
        for idx in 0..160 {
            let col = COLOURS[0];
            gg.ppu().set_pixel(idx, y, [col, col, col, 255]);
        }
    }

    pub fn allow_obj(&mut self, x: u8, count: u8) -> bool {
        match &mut self.kind {
            PpuKind::Dmg { used_x_obj_coords }
            | PpuKind::Cgb(Cgb {
                dmg_used_x_obj_cords: Some(used_x_obj_coords),
                ..
            }) => {
                if used_x_obj_coords
                    .iter()
                    .take(count.us())
                    .any(|e| *e == Some(x))
                {
                    return false;
                }
                used_x_obj_coords[count.us()] = Some(x);
                true
            }

            PpuKind::Cgb(_) => true,
        }
    }

    pub fn get_bg_colours(gg: &GameGirl) -> [Colour; 4] {
        let palette = gg[BGP];
        [
            Self::get_colour(palette, 0),
            Self::get_colour(palette, 1),
            Self::get_colour(palette, 2),
            Self::get_colour(palette, 3),
        ]
    }

    pub fn get_colour(palette: u8, colour: u8) -> Colour {
        let col = COLOURS[((palette >> (colour * 2)) & 0b11).us()];
        [col, col, col, 255]
    }
}
