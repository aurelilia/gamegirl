use crate::gui::Colour;
use crate::numutil::NumExt;
use crate::system::io::addr::BGP;
use crate::system::io::ppu::Ppu;
use crate::GameGirl;

const COLOURS: [u8; 4] = [255, 191, 63, 0];

impl Ppu {
    pub fn clear_line(gg: &mut GameGirl) {}

    pub fn get_bg_colour(gg: &GameGirl, colour: u16) -> Colour {
        Self::get_colour(gg.mmu[BGP], colour)
    }

    fn get_colour(palette: u8, colour: u16) -> Colour {
        Colour::from_gray(COLOURS[((palette >> (colour * 2)) & 0b11).us()])
    }
}
