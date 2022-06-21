use crate::{
    gga::{
        addr::{BG0CNT, DISPCNT},
        graphics::{Ppu, BG0_EN, WIN0_EN, WIN1_EN},
        GameGirlAdv,
    },
    numutil::{hword, NumExt},
};

const BG_X_SIZE: [u16; 4] = [32, 64, 32, 64];
const BG_Y_SIZE: [u16; 4] = [32, 32, 64, 64];

impl Ppu {
    pub fn render_mode0(gg: &mut GameGirlAdv, line: u16) {
        for prio in (0..4).rev() {
            Self::render_bg_text::<3>(gg, line, prio);
            Self::render_bg_text::<2>(gg, line, prio);
            Self::render_bg_text::<1>(gg, line, prio);
            Self::render_bg_text::<0>(gg, line, prio);
        }

        if gg[DISPCNT].is_bit(WIN1_EN) {}
        if gg[DISPCNT].is_bit(WIN0_EN) {}

        Self::render_objs::<0>(gg, line);
    }

    fn render_bg_text<const IDX: u16>(gg: &mut GameGirlAdv, line: u16, prio: u16) {
        if !gg[DISPCNT].is_bit(BG0_EN + IDX) {
            return;
        }
        let cnt = gg[BG0CNT + (IDX.u32() * 2)];
        if (cnt & 3) != prio {
            return;
        }
        Self::render_bg_text_section::<IDX>(gg, line, cnt, 0, 240);
    }

    fn render_bg_text_section<const IDX: u16>(
        gg: &mut GameGirlAdv,
        line: u16,
        cnt: u16,
        _startx: u16,
        _endx: u16,
    ) {
        let tile_y = line >> 3;
        let size = cnt.bits(14, 2).us();
        let (x_size, y_size) = (BG_X_SIZE[size], BG_Y_SIZE[size]);

        let base_tile_addr = cnt.bits(2, 2).us() * 0x4000;
        let mut map_addr = (cnt.bits(8, 5).us() * 0x800) + (x_size.us() * tile_y.us() * 2);
        let col_256 = cnt.is_bit(7);

        let tile_line = line & 7;
        for map_idx in 0..32 {
            let map = hword(gg.ppu.vram[map_addr], gg.ppu.vram[map_addr + 1]);
            let tile_idx = map.bits(0, 10);

            let line_addr = base_tile_addr + (tile_idx.us() * 32) + (tile_line.us() * 4);
            if col_256 {
                Self::render_bg_tile_256pal(gg, line, line_addr, map_idx * 8);
            } else {
                let pal_offs = map.bits(12, 4).u8() << 4;
                Self::render_bg_tile_16pal(gg, line, line_addr, map_idx * 8, pal_offs);
            }

            map_addr += 2;
        }
    }

    fn render_bg_tile_16pal(
        gg: &mut GameGirlAdv,
        line: u16,
        tile_addr: usize,
        mut x_pos: u16,
        pal_offs: u8,
    ) {
        for pair in 0..4 {
            let dat = gg.ppu.vram[tile_addr + pair];
            gg.ppu.set_pixel::<false>(line, x_pos, pal_offs, dat & 0x0F);
            gg.ppu
                .set_pixel::<false>(line, x_pos + 1, pal_offs, dat >> 4);
            x_pos += 2;
        }
    }

    fn render_bg_tile_256pal(gg: &mut GameGirlAdv, line: u16, tile_addr: usize, mut x_pos: u16) {
        for pix in 0..8 {
            let dat = gg.ppu.vram[tile_addr + pix];
            gg.ppu.set_pixel::<false>(line, x_pos, 0, dat);
            x_pos += 1;
        }
    }
}
