use crate::numutil::NumExt;
use crate::system::io::addr::BGP;
use crate::system::io::ppu::Ppu;
use crate::system::GameGirl;
use crate::Colour;

const COLOURS: [u8; 4] = [255, 191, 63, 0];

impl Ppu {
    pub fn clear_line(gg: &mut GameGirl) {}

    pub fn get_bg_colours(gg: &GameGirl) -> [Colour; 4] {
        let palette = gg.mmu[BGP];
        [
            Self::get_colour(palette, 0),
            Self::get_colour(palette, 1),
            Self::get_colour(palette, 2),
            Self::get_colour(palette, 3),
        ]
    }

    fn get_colour(palette: u8, colour: u16) -> Colour {
        Colour::from_gray(COLOURS[((palette >> (colour * 2)) & 0b11).us()])
    }
}
