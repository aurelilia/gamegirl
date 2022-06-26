use crate::{
    gga::{addr::MOSAIC, graphics::Ppu, GameGirlAdv},
    numutil::NumExt,
    Colour,
};

impl Ppu {
    pub(super) fn render_tile_4bpp<const OBJ: bool>(
        gg: &mut GameGirlAdv,
        prio: u16,
        mut x: i16,
        x_step: i16,
        tile_addr: usize,
        palette: u8,
        mosaic: bool,
    ) {
        for idx in 0..4 {
            let byte = gg.ppu.vram(tile_addr + idx);
            Self::set_pixel::<OBJ>(gg, x, prio, palette, byte & 0xF, mosaic);
            x += x_step;
            Self::set_pixel::<OBJ>(gg, x, prio, palette, byte >> 4, mosaic);
            x += x_step;
        }
    }

    pub(super) fn render_tile_8bpp<const OBJ: bool>(
        gg: &mut GameGirlAdv,
        prio: u16,
        mut x: i16,
        x_step: i16,
        tile_addr: usize,
        mosaic: bool,
    ) {
        for idx in 0..8 {
            let colour = gg.ppu.vram(tile_addr + idx);
            Self::set_pixel::<OBJ>(gg, x, prio, 0, colour, mosaic);
            x += x_step;
        }
    }

    fn get_pixel<const OBJ: bool>(&mut self, x: u16, prio: u16) -> Option<Colour> {
        if !(0..240).contains(&x) {
            return None;
        }
        let layers = if OBJ {
            &mut self.obj_layers
        } else {
            &mut self.bg_layers
        };
        Some(layers[prio.us()][x.us()])
    }

    fn set_pixel<const OBJ: bool>(
        gg: &mut GameGirlAdv,
        x: i16,
        prio: u16,
        palette: u8,
        colour_idx: u8,
        mosaic: bool,
    ) {
        if !(0..240).contains(&x) || colour_idx == 0 {
            return;
        }
        let x = x as u16;

        if mosaic {
            let stretch = if OBJ {
                gg[MOSAIC].bits(8, 4)
            } else {
                gg[MOSAIC].bits(0, 4)
            };
            if stretch != 0 && x % stretch != 0 {
                let actual_x = x - (x % stretch);
                if let Some(colour) = gg.ppu.get_pixel::<OBJ>(actual_x, prio) {
                    let layers = Self::get_layers::<OBJ>(gg);
                    layers[prio.us()][x.us()] = colour;
                    return;
                }
            }
        }

        let colour = gg.ppu.idx_to_palette::<OBJ>((palette << 4) + colour_idx);
        let layers = Self::get_layers::<OBJ>(gg);
        layers[prio.us()][x.us()] = colour;
    }

    fn get_layers<const OBJ: bool>(gg: &mut GameGirlAdv) -> &mut [[Colour; 240]; 4] {
        if OBJ {
            &mut gg.ppu.obj_layers
        } else {
            &mut gg.ppu.bg_layers
        }
    }

    fn vram(&self, addr: usize) -> u8 {
        if addr <= 0x17FFF {
            self.vram[addr]
        } else {
            self.vram[addr - 0x18000]
        }
    }
}
