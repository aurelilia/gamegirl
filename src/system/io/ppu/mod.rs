use crate::system::io::addr;

mod cgb;
mod dmg;

pub struct PPU {
    mode: Mode,
    mode_clock: u16,
    bg_occupied_pixels: [bool; 160 * 144],
}

impl PPU {}

impl Default for PPU {
    fn default() -> Self {
        Self {
            mode: Mode::HBlank,
            mode_clock: 0,
            bg_occupied_pixels: [false; 160 * 144],
        }
    }
}

#[derive(Copy, Clone)]
enum Mode {
    HBlank = 204,
    VBlank = 456,
    OAMScan = 80,
    Upload = 172,
}

impl Mode {
    fn cycles(self) -> u16 {
        self as u16
    }
}
