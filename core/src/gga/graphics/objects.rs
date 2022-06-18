use crate::{
    gga::{
        addr::DISPCNT,
        graphics::{Ppu, OBJ_EN},
        GameGirlAdv,
    },
    numutil::{hword, NumExt},
    Colour,
};

const OBJ_X_SIZE: [u16; 12] = [8, 16, 32, 64, 16, 32, 32, 64, 8, 8, 16, 32];
const OBJ_Y_SIZE: [u16; 12] = [8, 16, 32, 64, 8, 8, 16, 32, 16, 32, 32, 64];

impl Ppu {
    pub fn render_objs<const _START: u16>(gg: &mut GameGirlAdv, line: u16) {
        if !gg[DISPCNT].is_bit(OBJ_EN) {
            return;
        }

        for idx in 0..127 {
            let addr = idx << 3;
            let obj = Object {
                x: gg.ppu.oam[addr + 2].u16() + ((gg.ppu.oam[addr + 3].u16() & 1) << 8),
                y: gg.ppu.oam[addr].u16(),
                attr0: gg.ppu.oam[addr + 1],
                attr1: gg.ppu.oam[addr + 3],
                attr2: hword(gg.ppu.oam[addr + 4], gg.ppu.oam[addr + 5]),
            };
            Self::render_obj(gg, line, obj);
        }
    }

    fn render_obj(gg: &mut GameGirlAdv, line: u16, obj: Object) {
        let size = obj.size();
        if obj.y > line || (obj.y + size.1) <= line {
            return; // Not on this line or disabled, or hack to prevent OOB
                    // pixel writes
        }
        let obj_y = line - obj.y;

        let tile_line = obj_y & 7;
        let mut x_pos = obj.x;
        let base_tile_idx = obj.attr2.bits(0, 10).us();
        let adj_tile_idx = base_tile_idx + ((obj_y.us() >> 3) * (size.0.us() >> 3));
        let mut tile_addr = 0x1_0000 + (adj_tile_idx * 32) + (tile_line.us() * 4);

        if obj.attr0.is_bit(5) {
            for _ in 0..(size.0 >> 3) {
                Self::render_obj_tile_256pal(gg, line, &mut tile_addr, &mut x_pos);
                if x_pos >= 240 {
                    break;
                }
            }
        } else {
            for _ in 0..(size.0 >> 3) {
                Self::render_obj_tile_16pal(
                    gg,
                    line,
                    &mut tile_addr,
                    &mut x_pos,
                    (obj.attr2.bits(12, 4) << 4).u8(),
                );
                if x_pos >= 240 {
                    break;
                }
            }
        }
    }

    fn render_obj_tile_256pal(
        gg: &mut GameGirlAdv,
        line: u16,
        tile_addr: &mut usize,
        x_pos: &mut u16,
    ) {
        for pix in 0..8 {
            let dat = gg.ppu.vram[*tile_addr + pix];
            gg.ppu
                .set_pixel(line, *x_pos, gg.ppu.idx_to_palette::<true>(dat));
            *x_pos += 1;
        }

        *tile_addr += 64;
    }

    fn render_obj_tile_16pal(
        gg: &mut GameGirlAdv,
        line: u16,
        tile_addr: &mut usize,
        x_pos: &mut u16,
        pal_offs: u8,
    ) {
        for pair in 0..4 {
            let dat = gg.ppu.vram[*tile_addr + pair];
            gg.ppu.set_pixel(
                line,
                *x_pos,
                gg.ppu.idx_to_palette::<true>(pal_offs + (dat & 0x0F)),
            );
            gg.ppu.set_pixel(
                line,
                *x_pos + 1,
                gg.ppu.idx_to_palette::<true>(pal_offs + (dat >> 4)),
            );
            *x_pos += 2;
        }

        *tile_addr += 32;
    }

    fn set_pixel(&mut self, y: u16, x: u16, colour: Colour) {
        if x >= 240 {
            return;
        }
        let addr = (y * 240) + x;
        self.pixels[addr.us()] = colour;
    }
}

struct Object {
    x: u16,
    y: u16,
    attr0: u8,
    attr1: u8,
    attr2: u16,
}

impl Object {
    fn size(&self) -> (u16, u16) {
        let addr = (self.attr1.bits(6, 2) | (self.attr0.bits(6, 2) << 2)).us();
        (OBJ_X_SIZE[addr], OBJ_Y_SIZE[addr])
    }
}
