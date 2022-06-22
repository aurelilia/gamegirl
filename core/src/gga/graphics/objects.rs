use crate::{
    gga::{
        addr::{DISPCNT, MOSAIC},
        graphics::{Ppu, OBJ_EN, OBJ_MAPPING_1D},
        GameGirlAdv,
    },
    numutil::{hword, NumExt},
};

const OBJ_X_SIZE: [u16; 16] = [8, 16, 32, 64, 16, 32, 32, 64, 8, 8, 16, 32, 0, 0, 0, 0];
const OBJ_Y_SIZE: [u16; 16] = [8, 16, 32, 64, 8, 8, 16, 32, 16, 32, 32, 64, 0, 0, 0, 0];

impl Ppu {
    pub fn render_objs<const _START: u16>(gg: &mut GameGirlAdv, line: u16) {
        if !gg[DISPCNT].is_bit(OBJ_EN) {
            return;
        }

        let is_2d = !gg[DISPCNT].is_bit(OBJ_MAPPING_1D);
        for idx in 0..127 {
            let addr = idx << 3;
            let y = gg.ppu.oam[addr] as i16;
            let obj = Object {
                x: gg.ppu.oam[addr + 2].u16() + ((gg.ppu.oam[addr + 3].u16() & 1) << 8),
                y: if y > 240 { y.wrapping_sub(0x100) } else { y },
                attr0: gg.ppu.oam[addr + 1],
                attr1: gg.ppu.oam[addr + 3],
                attr2: hword(gg.ppu.oam[addr + 4], gg.ppu.oam[addr + 5]),
            };
            Self::render_obj(gg, line, obj, is_2d);
        }
    }

    fn render_obj(gg: &mut GameGirlAdv, line: u16, obj: Object, is_2d: bool) {
        if !obj.draw_on(line) {
            return;
        }
        let (mut obj_x, x_step) = obj.signed_x();
        let obj_y = obj.y_on(line, gg[MOSAIC]);
        let tile_y = obj_y & 7;

        // TODO: Object modes.
        let size = obj.size();
        let base_tile_idx = obj.attr2.bits(0, 10).us();
        let adj_tile_idx =
            base_tile_idx + ((obj_y.us() >> 3) * if !is_2d { size.0.us() >> 3 } else { 32 });
        let tile_addr = 0x1_0000 + (adj_tile_idx * 32);

        let tile_count = size.0 >> 3;
        let prio = obj.attr2.bits(10, 2);
        let mosaic = obj.attr2.is_bit(4);

        if obj.attr0.is_bit(5) {
            let mut tile_line_addr = tile_addr + (tile_y.us() * 8);
            for _ in 0..tile_count {
                Self::render_tile_8bpp::<true>(gg, prio, obj_x, x_step, tile_line_addr, mosaic);
                obj_x += x_step * 8;
                tile_line_addr += 64;
            }
        } else {
            let mut tile_line_addr = tile_addr + (tile_y.us() * 4);
            let palette = obj.attr2.bits(12, 4).u8();
            for _ in 0..tile_count {
                Self::render_tile_4bpp::<true>(
                    gg,
                    prio,
                    obj_x,
                    x_step,
                    tile_line_addr,
                    palette,
                    mosaic,
                );
                obj_x += x_step * 8;
                tile_line_addr += 32;
            }
        }
    }
}

struct Object {
    x: u16,
    y: i16,
    attr0: u8,
    attr1: u8,
    attr2: u16,
}

impl Object {
    fn size(&self) -> (u16, u16) {
        let addr = (self.attr1.bits(6, 2) | (self.attr0.bits(6, 2) << 2)).us();
        (OBJ_X_SIZE[addr], OBJ_Y_SIZE[addr])
    }

    fn draw_on(&self, line: u16) -> bool {
        self.valid() && !(self.y > line as i16 || (self.y + self.size().1 as i16) <= line as i16)
    }

    fn valid(&self) -> bool {
        self.attr0.bits(3, 2) != 3 && self.attr0.bits(6, 2) != 3
    }

    fn y_on(&self, line: u16, mosaic: u16) -> u16 {
        let mut pos = line.wrapping_add_signed(-self.y);
        // Consider VFlip and Mosaic
        if self.attr0.is_bit(4) {
            pos &= mosaic.bits(12, 4) - 1;
        }
        if self.attr1.is_bit(5) {
            pos = self.size().1 - pos - 1;
        }
        pos
    }

    fn signed_x(&self) -> (i16, i16) {
        let x = if self.x.is_bit(8) {
            // i didn't pay attention in math class
            -(0xFF - (self.x as i16 & 0xFF))
        } else {
            self.x as i16 & 0xFF
        };
        if self.attr1.is_bit(4) {
            (x + self.size().0 as i16 - 1, -1)
        } else {
            (x, 1)
        }
    }
}
