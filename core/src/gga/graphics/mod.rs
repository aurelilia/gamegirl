// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! For this PPU implementation, I took a lot of reference from DenSinH's GBAC-.
//! It is not an outright copy, but I want to thank them for their code
//! that helped me understand the PPU's more complex behavior.
//! The code is under the MIT license at https://github.com/DenSinH/GBAC-.

mod bitmap;
mod objects;
mod palette;
mod render;
mod tile;

use serde::{Deserialize, Serialize};

use super::memory::KB;
use crate::{
    common::BorrowedSystem,
    gga::{
        addr::{BG2PA, BG3PA, DISPCNT, DISPSTAT, VCOUNT, WIN0H, WIN0V, WININ, WINOUT},
        cpu::{Cpu, Interrupt},
        dma::{DmaReason, Dmas},
        scheduling::{AdvEvent, PpuEvent},
        GameGirlAdv,
    },
    numutil::{NumExt, U16Ext},
    Colour,
};

// DISPCNT
const FRAME_SELECT: u16 = 4;
const _OAM_HBLANK_FREE: u16 = 5;
const OBJ_MAPPING_1D: u16 = 6;
const FORCED_BLANK: u16 = 7;
const BG0_EN: u16 = 8;
const BG2_EN: u16 = 10;
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

type Layer = [Colour; 240];
type WindowMask = [bool; 240];

#[derive(Deserialize, Serialize)]
pub struct Ppu {
    #[serde(with = "serde_arrays")]
    pub palette: [u8; KB],
    #[serde(with = "serde_arrays")]
    pub vram: [u8; 96 * KB],
    #[serde(with = "serde_arrays")]
    pub oam: [u8; KB],

    #[serde(skip)]
    #[serde(default = "serde_colour_arr")]
    pixels: [Colour; 240 * 160],
    #[serde(skip)]
    #[serde(default = "serde_layer_arr")]
    bg_layers: [Layer; 4],
    #[serde(skip)]
    #[serde(default = "serde_layer_arr")]
    obj_layers: [Layer; 4],

    #[serde(skip)]
    #[serde(default = "serde_mask_arr")]
    win_masks: [WindowMask; 5],

    bg_x: [i32; 2],
    bg_y: [i32; 2],

    #[serde(skip)]
    #[serde(default)]
    pub last_frame: Option<Vec<Colour>>,
}

impl Ppu {
    pub fn handle_event(gg: &mut GameGirlAdv, event: PpuEvent, late_by: i32) {
        let (next_event, cycles) = match event {
            PpuEvent::HblankStart => {
                Self::render_line(gg);
                Self::maybe_interrupt(gg, Interrupt::HBlank, HBLANK_IRQ);
                Dmas::update_all(gg, DmaReason::HBlank);

                (PpuEvent::SetHblank, 46i32)
            }

            PpuEvent::SetHblank => {
                gg[DISPSTAT] = gg[DISPSTAT].set_bit(HBLANK, true);
                (PpuEvent::HblankEnd, 226)
            }

            PpuEvent::HblankEnd => {
                gg[VCOUNT] += 1;

                if gg[VCOUNT] == gg[DISPSTAT].bits(8, 8) {
                    gg[DISPSTAT] = gg[DISPSTAT].set_bit(LYC_MATCH, true);
                    Self::maybe_interrupt(gg, Interrupt::VCounter, LYC_IRQ);
                } else {
                    gg[DISPSTAT] = gg[DISPSTAT].set_bit(LYC_MATCH, false);
                }

                match gg[VCOUNT] {
                    160 => {
                        gg[DISPSTAT] = gg[DISPSTAT].set_bit(VBLANK, true);
                        Self::maybe_interrupt(gg, Interrupt::VBlank, VBLANK_IRQ);
                        Dmas::update_all(gg, DmaReason::VBlank);
                        Self::reload_affine_bgs(gg);
                        gg.ppu.last_frame = Some(gg.ppu.pixels.to_vec());
                    }
                    // VBlank flag gets set one scanline early
                    227 => gg[DISPSTAT] = gg[DISPSTAT].set_bit(VBLANK, false),
                    228 => {
                        gg[VCOUNT] = 0;
                        (gg.options.frame_finished)(BorrowedSystem::GGA(gg));
                    }
                    _ => (),
                }

                gg[DISPSTAT] = gg[DISPSTAT].set_bit(HBLANK, false);
                (PpuEvent::HblankStart, 960)
            }
        };
        gg.scheduler
            .schedule(AdvEvent::PpuEvent(next_event), cycles - late_by);
    }

    fn maybe_interrupt(gg: &mut GameGirlAdv, int: Interrupt, bit: u16) {
        if gg[DISPSTAT].is_bit(bit) {
            Cpu::request_interrupt(gg, int);
        }
    }

    fn render_line(gg: &mut GameGirlAdv) {
        let line = gg[VCOUNT];
        if line >= 160 {
            return;
        }
        if gg[DISPCNT].is_bit(FORCED_BLANK) {
            let start = line.us() * 240;
            for pixel in 0..240 {
                gg.ppu.pixels[start + pixel] = [31, 31, 31, 255];
            }
            return;
        }

        Self::calc_windows(gg, line.u8());
        match gg[DISPCNT] & 7 {
            0 => Self::render_mode0(gg, line),
            1 => Self::render_mode1(gg, line),
            2 => Self::render_mode2(gg, line),
            3 => Self::render_mode3(gg, line),
            4 => Self::render_mode4(gg, line),
            5 => Self::render_mode5(gg, line),
            _ => println!("Invalid mode {}", gg[DISPCNT] & 7),
        }

        Self::finish_line(gg, line);
    }

    fn calc_windows(gg: &mut GameGirlAdv, line: u8) {
        let cnt = gg[DISPCNT];
        let win0_en = cnt.is_bit(WIN0_EN);
        let win1_en = cnt.is_bit(WIN1_EN);
        let win_obj_en = cnt.is_bit(WIN_OBJS);
        if !win0_en && !win1_en && !win_obj_en {
            // No windows enabled, allow everything to draw
            gg.ppu.win_masks = serde_mask_arr();
            return;
        }

        let win = gg[WININ];
        let wout = gg[WINOUT];

        // First set parameters for the outside region, anything in windows
        // will be overwritten later.
        for mask in 0..5 {
            let enable = wout.is_bit(mask);
            gg.ppu.win_masks[mask.us()] = [enable; 240];
        }

        // Do window 1 first, since it has lower priority and we want
        // window 0 to overwrite it
        for window in (0..2).rev() {
            if !cnt.is_bit(WIN0_EN + window.u16()) {
                // Window disabled.
                continue;
            }

            let win_offs = (window * 8).u16();
            let (x1, x2) = Self::window_coords::<240>(gg[WIN0H + window * 2]);
            let (y1, y2) = Self::window_coords::<160>(gg[WIN0V + window * 2]);

            if !(y1..y2).contains(&line) {
                // Window outside this line.
                continue;
            }

            for mask in 0..5 {
                let enable = win.is_bit(win_offs + mask);
                for pixel in x1..x2 {
                    gg.ppu.win_masks[mask.us()][pixel.us()] = enable;
                }
            }
        }
    }

    // Return WIN0/WIN1 coordinates, first is start, second is end.
    fn window_coords<const MAX: u8>(value: u16) -> (u8, u8) {
        let x1 = value.high();
        let x2 = value.low().saturating_add(1).min(MAX);
        if x1 > x2 {
            (x1, MAX)
        } else {
            (x1, x2)
        }
    }

    fn finish_line(gg: &mut GameGirlAdv, line: u16) {
        let start = line.us() * 240;
        let mut backdrop = gg.ppu.idx_to_palette::<false>(0); // BG0 is backdrop
        Self::adjust_pixel(&mut backdrop);

        'pixels: for (x, pixel) in gg.ppu.pixels[start..].iter_mut().take(240).enumerate() {
            for prio in 0..4 {
                if gg.ppu.obj_layers[prio][x] != EMPTY {
                    *pixel = gg.ppu.obj_layers[prio][x];
                    Self::adjust_pixel(pixel);
                    continue 'pixels;
                }
                if gg.ppu.bg_layers[prio][x] != EMPTY {
                    *pixel = gg.ppu.bg_layers[prio][x];
                    Self::adjust_pixel(pixel);
                    continue 'pixels;
                }
            }

            // No matching pixel found...
            *pixel = backdrop;
        }

        // Clear last line buffers
        gg.ppu.bg_layers = serde_layer_arr();
        gg.ppu.obj_layers = serde_layer_arr();
    }

    fn reload_affine_bgs(gg: &mut GameGirlAdv) {
        gg.ppu.bg_x[0] = Self::get_affine_offs(gg[BG2PA + 0x8], gg[BG2PA + 0xA]);
        gg.ppu.bg_y[0] = Self::get_affine_offs(gg[BG2PA + 0xC], gg[BG2PA + 0xE]);
        gg.ppu.bg_x[1] = Self::get_affine_offs(gg[BG3PA + 0x8], gg[BG3PA + 0xA]);
        gg.ppu.bg_y[1] = Self::get_affine_offs(gg[BG3PA + 0xC], gg[BG3PA + 0xE]);
    }

    #[inline]
    fn adjust_pixel(pixel: &mut Colour) {
        for col in pixel.iter_mut().take(3) {
            *col = (*col << 3) | (*col >> 2);
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            palette: [0; KB],
            vram: [0; 96 * KB],
            oam: [0; KB],

            bg_x: [0; 2],
            bg_y: [0; 2],

            pixels: serde_colour_arr(),
            bg_layers: serde_layer_arr(),
            obj_layers: serde_layer_arr(),
            win_masks: serde_mask_arr(),
            last_frame: None,
        }
    }
}

fn serde_colour_arr() -> [Colour; 240 * 160] {
    [[0, 0, 0, 255]; 240 * 160]
}
fn serde_layer_arr() -> [Layer; 4] {
    [[EMPTY; 240]; 4]
}
fn serde_mask_arr() -> [WindowMask; 5] {
    [[true; 240]; 5]
}

const EMPTY: Colour = [0, 0, 0, 0];
