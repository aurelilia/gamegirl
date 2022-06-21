use crate::{
    gga::{
        addr::*,
        graphics::{Ppu, BG2_EN, FRAME_SELECT},
        GameGirlAdv,
    },
    numutil::NumExt,
};

impl Ppu {
    pub fn render_mode3(gg: &mut GameGirlAdv, line: u16) {
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

    pub fn render_mode4(gg: &mut GameGirlAdv, line: u16) {
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

    pub fn render_mode5(gg: &mut GameGirlAdv, line: u16) {
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

    fn bitmap_start_addr(gg: &GameGirlAdv) -> usize {
        if !gg[DISPCNT].is_bit(FRAME_SELECT) {
            0x0
        } else {
            0xA000
        }
    }
}
