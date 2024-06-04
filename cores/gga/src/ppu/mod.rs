// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! For this PPU implementation, I took a lot of reference from DenSinH's GBAC-.
//! It is not an outright copy, but I want to thank them for their code
//! that helped me understand the PPU's more complex behavior.
//! The code is under the MIT license at https://github.com/DenSinH/GBAC-.
//! Additionally RustBoyAdvance-ng by michelhe was heavily used for the
//! second attempt at an implementation. Thank you to michelhe, too.
//! The code is under the MIT license at  https://github.com/michelhe/rustboyadvance-ng.

mod modes;
mod objects;
mod palette;
mod registers;
mod render;
mod tile;

use std::ops::RangeInclusive;

use arm_cpu::{Cpu, Interrupt};
use common::{numutil::NumExt, Colour};
use objects::ObjPixel;
use registers::*;

use crate::{
    dma::{Dmas, Reason},
    memory::KB,
    scheduling::{AdvEvent, PpuEvent},
    GameGirlAdv,
};

#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct Point(i32, i32);

impl Point {
    fn inbounds(self, w: usize, h: usize) -> bool {
        let Point(x, y) = self;
        x >= 0 && x < w as i32 && y >= 0 && y < h as i32
    }
}

fn xy2d(x: usize, y: usize) -> usize {
    xy2dw(x, y, WIDTH)
}

fn xy2dw(x: usize, y: usize, w: usize) -> usize {
    (y * w) + x
}

const WIDTH: usize = 240;
const HEIGHT: usize = 160;
const VBLANK_END: u16 = 228;
const BUF: usize = WIDTH * HEIGHT;
const BLACK: Colour = [0, 0, 0, 255];
const WHITE: Colour = [31, 31, 31, 255];
const TRANS: Colour = [0, 0, 0, 0];

type Layer = [Colour; WIDTH];

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Ppu {
    // Registers
    pub dispcnt: DisplayControl,
    greepswap: GreenSwap,
    dispstat: DisplayStatus,
    pub(crate) vcount: u16,
    bg_cnt: [BgControl; 4],
    bg_offsets: [u16; 8],
    bg_scale: [BgRotScal; 2],

    windows: [Window; 2],
    win_obj: WindowCtrl,
    win_out: WindowCtrl,

    mosaic: Mosaic,
    bldcnt: BlendControl,
    bldalpha: BlendAlpha,
    bldy: u16,

    // Memory
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub palette: [u8; KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub vram: [u8; 96 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub oam: [u8; KB],

    // Last frame
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub last_frame: Option<Vec<Colour>>,

    // Current frame
    /// Pixels of the frame currently being constructed.
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_colour_arr"))]
    pixels: [Colour; BUF],
    /// Pixel output of each background layer.
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_layer_arr"))]
    bg_layers: [Layer; 4],
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_obj_arr"))]
    obj_layer: [ObjPixel; WIDTH],
}

impl Ppu {
    pub fn handle_event(gg: &mut GameGirlAdv, event: PpuEvent, late_by: i64) {
        let (next_event, cycles) = match event {
            PpuEvent::HblankStart => {
                gg.ppu.render_line();
                Self::maybe_interrupt(gg, Interrupt::HBlank);
                Dmas::update_all(gg, Reason::HBlank);
                (PpuEvent::SetHblank, 46)
            }

            PpuEvent::SetHblank => {
                gg.ppu.dispstat.set_in_hblank(true);
                (PpuEvent::HblankEnd, 226)
            }

            PpuEvent::HblankEnd => {
                gg.ppu.vcount += 1;

                let vcount_match = gg.ppu.vcount.u8() == gg.ppu.dispstat.vcount();
                gg.ppu.dispstat.set_vcounter_match(vcount_match);
                if vcount_match {
                    Self::maybe_interrupt(gg, Interrupt::VCounter);
                }

                let vcount = gg.ppu.vcount;
                match () {
                    _ if gg.ppu.vcount == HEIGHT.u16() => {
                        gg.ppu.dispstat.set_in_vblank(true);
                        Self::maybe_interrupt(gg, Interrupt::VBlank);
                        Dmas::update_all(gg, Reason::VBlank);
                    }
                    // VBlank flag gets set one scanline early
                    _ if vcount == (VBLANK_END - 1) => {
                        gg.ppu.dispstat.set_in_vblank(false);
                    }
                    _ if vcount == VBLANK_END => {
                        gg.ppu.vcount = 0;
                        let frame = Self::end_frame(gg);
                        gg.ppu.last_frame = Some(frame);
                    }
                    _ => (),
                }

                gg.ppu.dispstat.set_in_hblank(false);
                (PpuEvent::HblankStart, 960)
            }
        };

        gg.scheduler
            .schedule(AdvEvent::PpuEvent(next_event), cycles - late_by);
    }

    fn maybe_interrupt(gg: &mut GameGirlAdv, int: Interrupt) {
        if gg.ppu.dispstat.irq_enables().is_bit(int as u16) {
            Cpu::request_interrupt(gg, int);
        }
    }

    fn render_line(&mut self) {
        if self.vcount >= HEIGHT.u16() {
            return;
        }
        if self.dispcnt.forced_blank_enable() {
            let start = self.vcount.us() * WIDTH;
            for pixel in 0..WIDTH {
                self.pixels[start + pixel] = [31, 31, 31, 255];
            }
            return;
        }

        if self.dispcnt.obj_en() {
            self.render_objs();
        }

        match self.dispcnt.bg_mode() {
            BackgroundMode::Mode0 => self.render_mode0(),
            BackgroundMode::Mode1 => self.render_mode1(),
            BackgroundMode::Mode2 => self.render_mode2(),
            BackgroundMode::Mode3 => self.render_mode3(),
            BackgroundMode::Mode4 => self.render_mode4(),
            BackgroundMode::Mode5 => self.render_mode5(),
            inv => log::warn!("Invalid PPU mode {inv:?}"),
        }

        // Update affines
        for bg in 2..4 {
            self.bg_scale[bg - 2].latched.0 += self.bg_scale[bg - 2].pb as i32;
            self.bg_scale[bg - 2].latched.1 += self.bg_scale[bg - 2].pd as i32;
        }
    }

    fn clean_buffers(&mut self, bgs: RangeInclusive<usize>) {
        for bg in bgs {
            self.bg_layers[bg] = [TRANS; WIDTH];
        }
        self.obj_layer = serde_obj_arr();
    }

    fn end_frame(gg: &mut GameGirlAdv) -> Vec<[u8; 4]> {
        // Reload affine backgrounds
        gg.ppu.bg_scale[0].latch();
        gg.ppu.bg_scale[1].latch();
        // That's it. Frame ready
        gg.ppu.pixels.to_vec()
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            dispcnt: Default::default(),
            greepswap: Default::default(),
            dispstat: Default::default(),
            vcount: Default::default(),
            bg_cnt: Default::default(),
            bg_offsets: Default::default(),
            bg_scale: Default::default(),
            windows: Default::default(),
            win_obj: Default::default(),
            win_out: Default::default(),
            mosaic: Default::default(),
            bldcnt: Default::default(),
            bldalpha: Default::default(),
            bldy: Default::default(),

            palette: [0; KB],
            vram: [0; 96 * KB],
            oam: [0; KB],

            pixels: serde_colour_arr(),
            bg_layers: serde_layer_arr(),
            obj_layer: serde_obj_arr(),
            last_frame: None,
        }
    }
}

fn serde_colour_arr() -> [Colour; BUF] {
    [TRANS; BUF]
}
fn serde_layer_arr() -> [Layer; 4] {
    [[TRANS; WIDTH]; 4]
}
fn serde_obj_arr() -> [ObjPixel; WIDTH] {
    [ObjPixel::default(); WIDTH]
}
