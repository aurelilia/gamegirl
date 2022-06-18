mod mode;
mod objects;
mod palette;

use super::memory::KB;
use crate::{
    common::BorrowedSystem,
    gga::{
        addr::{DISPCNT, DISPSTAT, VCOUNT},
        cpu::{Cpu, Interrupt},
        GameGirlAdv,
    },
    numutil::{NumExt, U16Ext},
    Colour,
};
use serde::{Deserialize, Serialize};

// DISPCNT
const FRAME_SELECT: u16 = 4;
const OAM_HBLANK_FREE: u16 = 5;
const OBJ_MAPPING_1D: u16 = 6;
const FORCED_BLANK: u16 = 7;
const BG0_EN: u16 = 8;
const BG1_EN: u16 = 9;
const BG2_EN: u16 = 10;
const BG3_EN: u16 = 11;
const OBJ_EN: u16 = 12;
const WIN0_EN: u16 = 13;
const WIN1_EN: u16 = 14;
const WIN_OBJS: u16 = 15;

// DISPSTAT
pub const VBLANK: u16 = 0;
pub const HBLANK: u16 = 1;
const LYC_MATCH: u16 = 2;
const VBLANK_IRQ: u16 = 3;
const HBLANK_IRQ: u16 = 4;
const LYC_IRQ: u16 = 5;

#[derive(Deserialize, Serialize)]
pub struct Ppu {
    #[serde(with = "serde_arrays")]
    pub palette: [u8; KB],
    #[serde(with = "serde_arrays")]
    pub vram: [u8; 96 * KB],
    #[serde(with = "serde_arrays")]
    pub oam: [u8; KB],

    mode: Mode,
    mode_clock: u16,

    #[serde(skip)]
    #[serde(default = "serde_colour_arr")]
    pixels: [Colour; 240 * 160],
    #[serde(skip)]
    #[serde(default)]
    pub last_frame: Option<Vec<Colour>>,
}

impl Ppu {
    pub fn step(gg: &mut GameGirlAdv, cycles: u16) {
        gg.ppu.mode_clock += cycles;
        if gg.ppu.mode_clock < gg.ppu.mode as u16 {
            return;
        }
        gg.ppu.mode_clock -= gg.ppu.mode as u16;

        gg.ppu.mode = match gg.ppu.mode {
            Mode::Upload => {
                Self::render_line(gg);
                Self::maybe_interrupt(gg, Interrupt::HBlank, HBLANK_IRQ);
                gg[DISPSTAT] = gg[DISPSTAT].set_bit(HBLANK, true);
                Mode::HBlank
            }

            Mode::HBlank => {
                gg[VCOUNT] += 1;

                gg[DISPSTAT] = gg[DISPSTAT].set_bit(LYC_MATCH, false);
                if gg[DISPSTAT].is_bit(LYC_IRQ) && gg[VCOUNT] == gg[DISPSTAT].bits(8, 8) {
                    gg[DISPSTAT] = gg[DISPSTAT].set_bit(LYC_MATCH, true);
                    Self::maybe_interrupt(gg, Interrupt::VCounter, LYC_IRQ);
                }

                if gg[VCOUNT] == 160 {
                    gg[DISPSTAT] = gg[DISPSTAT].set_bit(VBLANK, true);
                    Self::maybe_interrupt(gg, Interrupt::VBlank, VBLANK_IRQ);
                    gg.ppu.last_frame = Some(Self::correct_colours(gg.ppu.pixels.to_vec()));
                    gg.ppu.pixels = [[31, 31, 31, 255]; 240 * 160];
                } else if gg[VCOUNT] > 227 {
                    gg[VCOUNT] = 0;
                    gg[DISPSTAT] = gg[DISPSTAT].set_bit(VBLANK, false);
                    (gg.options.frame_finished)(BorrowedSystem::GGA(gg));
                }

                gg[DISPSTAT] = gg[DISPSTAT].set_bit(HBLANK, false);
                Mode::Upload
            }
        }
    }

    fn maybe_interrupt(gg: &mut GameGirlAdv, int: Interrupt, bit: u16) {
        if gg[DISPSTAT].is_bit(bit) {
            Cpu::request_interrupt(gg, int);
        }
    }

    fn render_line(gg: &mut GameGirlAdv) {
        let line = gg[VCOUNT];
        if line >= 160 || gg[DISPCNT].is_bit(FORCED_BLANK) {
            return;
        }

        match gg[DISPCNT] & 7 {
            0 => Self::render_mode0(gg, line),
            3 => Self::render_mode3(gg, line),
            4 => Self::render_mode4(gg, line),
            5 => Self::render_mode5(gg, line),
            _ => println!("Unimplemented mode {}", gg[DISPCNT] & 7),
        }
    }

    /// Map a Vec of colours in 0-31 GGA space to 0-255 RGB.
    fn correct_colours(mut pixels: Vec<Colour>) -> Vec<Colour> {
        for pixel in pixels.iter_mut() {
            for col in pixel.iter_mut().take(3) {
                *col = (*col << 3) | (*col >> 2);
            }
        }
        pixels
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            palette: [0; KB],
            vram: [0; 96 * KB],
            oam: [0; KB],
            mode: Mode::Upload,
            mode_clock: 0,
            pixels: serde_colour_arr(),
            last_frame: None,
        }
    }
}

#[derive(Copy, Clone, Deserialize, Serialize)]
enum Mode {
    Upload = 960,
    HBlank = 272,
}

fn serde_colour_arr() -> [Colour; 240 * 160] {
    [[0, 0, 0, 255]; 240 * 160]
}
