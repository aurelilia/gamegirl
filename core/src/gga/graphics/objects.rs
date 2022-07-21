// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    gga::{
        addr::{DISPCNT, MOSAIC, WINOUT},
        graphics::{threading::PpuType, Ppu, OBJ_EN, OBJ_MAPPING_1D, WIN_OBJS},
    },
    numutil::{hword, NumExt},
};

const OBJ_X_SIZE: [u16; 16] = [8, 16, 32, 64, 16, 32, 32, 64, 8, 8, 16, 32, 0, 0, 0, 0];
const OBJ_Y_SIZE: [u16; 16] = [8, 16, 32, 64, 8, 8, 16, 32, 16, 32, 32, 64, 0, 0, 0, 0];

impl Ppu {
    pub fn render_objs<const _START: u16>(gg: &mut PpuType, line: u16) {
        if !gg[DISPCNT].is_bit(OBJ_EN) {
            return;
        }

        let is_2d = !gg[DISPCNT].is_bit(OBJ_MAPPING_1D);
        for idx in 0..128 {
            let addr = idx << 3;
            let y = gg.ppu.oam[addr];
            let obj = Object {
                x: gg.ppu.oam[addr + 2].u16() + ((gg.ppu.oam[addr + 3].u16() & 1) << 8),
                y,
                attr0: gg.ppu.oam[addr + 1],
                attr1: gg.ppu.oam[addr + 3],
                attr2: hword(gg.ppu.oam[addr + 4], gg.ppu.oam[addr + 5]),
            };
            Self::render_obj(gg, line, obj, is_2d);
        }
    }

    fn render_obj(gg: &mut PpuType, line: u16, obj: Object, is_2d: bool) {
        match obj.attr0 & 3 {
            0 => Self::render_obj_regular(gg, line, obj, is_2d),
            1 => Self::render_obj_affine(gg, line, obj, is_2d, false),
            3 => Self::render_obj_affine(gg, line, obj, is_2d, true),
            _ => (), // OBJ disable on a regular OBJ
        }
    }

    fn render_obj_regular(gg: &mut PpuType, line: u16, obj: Object, is_2d: bool) {
        let size = obj.size();
        if !obj.draw_on(line, size.1.u8()) {
            return;
        }

        let (mut obj_x, x_step) = obj.signed_x::<true>();
        let obj_y = obj.y_on(line, gg[MOSAIC]);
        let tile_y = obj_y & 7;

        let base_tile_idx = obj.attr2.bits(0, 10).us();
        let tile_count = size.0 >> 3;
        let prio = obj.attr2.bits(10, 2);
        let mosaic = obj.attr2.is_bit(4);
        let is_window = gg[DISPCNT].is_bit(WIN_OBJS) && obj.attr0.bits(2, 2) == 2;

        if obj.attr0.is_bit(5) {
            let tile_addr = 0x1_0000
                + if !is_2d {
                    (base_tile_idx + (((obj_y.us() >> 3) * size.0.us()) >> 3)) * 64
                } else {
                    (base_tile_idx + ((obj_y.us() >> 3) * 32)) * 32
                };
            let mut tile_line_addr = tile_addr + (tile_y.us() * 8);
            for _ in 0..tile_count {
                if is_window {
                    for idx in 0..8 {
                        let colour = gg.ppu.vram(tile_addr + idx);
                        Self::set_window_pixel(gg, obj_x, colour);
                        obj_x += x_step;
                    }
                } else {
                    Self::render_tile_8bpp::<true>(
                        gg,
                        prio,
                        obj_x,
                        x_step,
                        tile_line_addr,
                        mosaic,
                        4,
                    );
                    obj_x += x_step * 8;
                    tile_line_addr += 64;
                }
            }
        } else {
            let adj_tile_idx =
                base_tile_idx + ((obj_y.us() >> 3) * if !is_2d { size.0.us() >> 3 } else { 32 });
            let tile_addr = 0x1_0000 + (adj_tile_idx * 32);
            let mut tile_line_addr = tile_addr + (tile_y.us() * 4);
            let palette = obj.attr2.bits(12, 4).u8();
            for _ in 0..tile_count {
                if is_window {
                    for idx in 0..4 {
                        let byte = gg.ppu.vram(tile_addr + idx);
                        Self::set_window_pixel(gg, obj_x, byte & 0xF);
                        obj_x += x_step;
                        Self::set_window_pixel(gg, obj_x, byte >> 4);
                        obj_x += x_step;
                    }
                } else {
                    Self::render_tile_4bpp::<true>(
                        gg,
                        prio,
                        obj_x,
                        x_step,
                        tile_line_addr,
                        palette,
                        mosaic,
                        4,
                    );
                    obj_x += x_step * 8;
                    tile_line_addr += 32;
                }
            }
        }
    }

    fn render_obj_affine(gg: &mut PpuType, line: u16, obj: Object, is_2d: bool, size_2x: bool) {
        let size = obj.size();
        if !obj.draw_on(line, if size_2x { size.1 << 1 } else { size.1 }.u8()) {
            return;
        }

        let (px0, py0) = (size.0 >> 1, size.1 >> 1);

        let (mut dx, dy) = if !size_2x {
            (
                -(px0 as i32),
                (line.u8()).wrapping_sub(obj.y) as i32 - py0 as i32,
            )
        } else {
            (
                -(size.0 as i32),
                (line.u8()).wrapping_sub(obj.y) as i32 - (size.1 as i32),
            )
        };
        let (obj_x, _) = obj.signed_x::<false>();
        let obj_width = size.0 * (1 + size_2x as u16);

        let base_tile_idx = obj.attr2.bits(0, 10).us();
        let prio = obj.attr2.bits(10, 2);
        let rotscal = gg.ppu.get_rotscal(obj.attr1.bits(1, 5));
        let (is_8bpp, palette) = if obj.attr0.is_bit(5) {
            (true, 0)
        } else {
            let palette = obj.attr2.bits(12, 4).u8();
            (false, palette)
        };
        let is_window = gg[DISPCNT].is_bit(WIN_OBJS) && obj.attr0.bits(2, 2) == 2;

        for x in 0..obj_width {
            let pixel_x = obj_x + x as i16;
            if pixel_x < 0 {
                continue;
            }
            if pixel_x >= 240 {
                break;
            }

            let trans_x = ((rotscal[0] * dx + rotscal[1] * dy) >> 8) + px0 as i32;
            let trans_y = ((rotscal[2] * dx + rotscal[3] * dy) >> 8) + py0 as i32;
            dx += 1;
            let (trans_x, trans_y) = (trans_x as u16, trans_y as u16);
            if trans_x >= size.0 || trans_y >= size.1 {
                continue;
            }

            // OBJ window
            let colour =
                Self::get_affine_pixel(gg, base_tile_idx, size, trans_x, trans_y, is_2d, is_8bpp);
            if is_window {
                Self::set_window_pixel(gg, pixel_x, colour);
            } else {
                Self::set_pixel::<true>(gg, pixel_x, prio, palette, colour, false, 4);
            }
        }
    }

    fn get_affine_pixel(
        gg: &mut PpuType,
        base_tile_idx: usize,
        size: (u16, u16),
        trans_x: u16,
        trans_y: u16,
        is_2d: bool,
        is_8bpp: bool,
    ) -> u8 {
        let tile_y = trans_y & 7;
        if is_8bpp {
            let tile_addr = 0x1_0000
                + if !is_2d {
                    (base_tile_idx + (((trans_y.us() >> 3) * size.0.us()) >> 3)) * 64
                } else {
                    (base_tile_idx + ((trans_y.us() >> 3) * 32)) * 32
                };
            let tile_line_addr = tile_addr + (tile_y.us() * 8) + (64 * (trans_x.us() >> 3));
            gg.ppu.vram(tile_line_addr + (trans_x.us() & 7))
        } else {
            let adj_tile_idx =
                base_tile_idx + ((trans_y.us() >> 3) * if !is_2d { size.0.us() >> 3 } else { 32 });
            let tile_addr = 0x1_0000 + (adj_tile_idx * 32);
            let tile_line_addr = tile_addr + (tile_y.us() * 4) + (32 * (trans_x.us() >> 3));
            let byte = gg.ppu.vram(tile_line_addr + ((trans_x.us() & 7) >> 1));
            if trans_x.is_bit(0) {
                byte >> 4
            } else {
                byte & 0xF
            }
        }
    }

    fn get_rotscal(&self, idx: u8) -> [i32; 4] {
        let mut offs = 32 * idx.us() + 6;
        let mut out = [0; 4];
        for elem in &mut out {
            *elem = hword(self.oam[offs], self.oam[offs + 1]) as i16 as i32;
            offs += 8;
        }
        out
    }

    fn set_window_pixel(gg: &mut PpuType, pixel: i16, colour: u8) {
        if !(0..240).contains(&pixel) || colour == 0 {
            return;
        }

        let wout = gg[WINOUT];
        for mask in 0..5 {
            let enable = wout.is_bit(8 + mask);
            gg.ppu.win_masks[mask.us()][pixel as usize] = enable;
        }
        gg.ppu.win_blend[pixel as usize] = wout.is_bit(8 + 5);
    }
}

#[derive(Copy, Clone)]
struct Object {
    x: u16,
    y: u8,
    attr0: u8,
    attr1: u8,
    attr2: u16,
}

impl Object {
    fn size(self) -> (u16, u16) {
        let addr = (self.attr1.bits(6, 2) | (self.attr0.bits(6, 2) << 2)).us();
        (OBJ_X_SIZE[addr], OBJ_Y_SIZE[addr])
    }

    fn draw_on(self, line: u16, size_y: u8) -> bool {
        let pos = line.u8().wrapping_sub(self.y);
        self.valid() && pos < size_y
    }

    fn valid(self) -> bool {
        self.attr0.bits(3, 2) != 3 && self.attr0.bits(6, 2) != 3
    }

    fn y_on(self, line: u16, mosaic: u16) -> u8 {
        let mut pos = line.u8().wrapping_sub(self.y);
        // Consider VFlip and Mosaic
        if self.attr0.is_bit(4) {
            pos &= (mosaic.bits(12, 4) - 1).u8();
        }
        if self.attr1.is_bit(5) {
            pos = self.size().1.u8() - pos - 1;
        }
        pos
    }

    fn signed_x<const FLIP: bool>(self) -> (i16, i16) {
        let x = if self.x.is_bit(8) {
            // i didn't pay attention in math class
            self.x as i16 | 0xFF00u16 as i16
        } else {
            self.x as i16
        };
        if FLIP && self.attr1.is_bit(4) {
            (x + self.size().0 as i16 - 1, -1)
        } else {
            (x, 1)
        }
    }
}
