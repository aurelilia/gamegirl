use crate::{
    gga::{
        addr::DISPCNT,
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
            gg.ppu.pixels[pixel] = gg.ppu.hword_to_colour_vram(pixel << 1);
        }
    }

    pub fn render_mode4(gg: &mut GameGirlAdv, line: u16) {
        if !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let line_start = line.us() * 240;
        let vram_start = Self::bitmap_start_addr(gg);
        for offs in 0..240 {
            let pixel = line_start + offs;
            let palette = gg.ppu.vram[vram_start + pixel];
            gg.ppu.pixels[pixel] = gg.ppu.idx_to_palette::<false>(palette);
        }
    }

    pub fn render_mode5(gg: &mut GameGirlAdv, line: u16) {
        if line > 127 || !gg[DISPCNT].is_bit(BG2_EN) {
            return;
        }

        let vram_start = Self::bitmap_start_addr(gg);
        let line_start = vram_start + (line.us() * 160);
        for offs in 0..160 {
            let pixel = (line_start + offs).us();
            gg.ppu.pixels[pixel] = gg.ppu.hword_to_colour_vram(pixel << 1);
        }
    }

    fn bitmap_start_addr(gg: &GameGirlAdv) -> usize {
        if gg[DISPCNT].is_bit(FRAME_SELECT) {
            0xA000
        } else {
            0x0
        }
    }
}

enum RotScal {
    Yes,
    Mixed,
    No,
}

impl RotScal {
    const MODES: [RotScal; 6] = [
        RotScal::No,
        RotScal::Mixed,
        RotScal::Yes,
        RotScal::Yes,
        RotScal::Yes,
        RotScal::Yes,
    ];
}
