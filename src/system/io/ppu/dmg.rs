use crate::numutil::NumExt;
use crate::system::io::addr::{BGP, LY};
use crate::system::io::ppu::{Ppu, PpuKind, Sprite};
use crate::system::GameGirl;
use crate::Colour;

const COLOURS: [u8; 4] = [255, 191, 63, 0];

impl Ppu {
    pub fn clear_line(gg: &mut GameGirl) {
        let y = gg.mmu[LY];
        for idx in 0..160 {
            gg.ppu().set_pixel(idx, y, Colour::from_gray(COLOURS[0]));
        }
    }

    pub fn allow_obj(&mut self, x: u8, count: u8) -> bool {
        match &mut self.kind {
            PpuKind::Dmg { used_x_obj_coords } => {
                for i in 0..count.us() {
                    if used_x_obj_coords[i] == Some(x) {
                        return false;
                    }
                }
                used_x_obj_coords[count.us()] = Some(x);
                true
            }

            PpuKind::Cgb => true,
        }
    }

    pub fn get_bg_colours(gg: &GameGirl) -> [Colour; 4] {
        let palette = gg.mmu[BGP];
        [
            Self::get_colour(palette, 0),
            Self::get_colour(palette, 1),
            Self::get_colour(palette, 2),
            Self::get_colour(palette, 3),
        ]
    }

    pub fn get_colour(palette: u8, colour: u16) -> Colour {
        Colour::from_gray(COLOURS[((palette >> (colour * 2)) & 0b11).us()])
    }
}
