// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! For this PPU implementation, I took a lot of reference from DenSinH's GBAC-.
//! It is not an outright copy, but I want to thank them for their code
//! that helped me understand the PPU's more complex behavior.
//! The code is under the MIT license at https://github.com/DenSinH/GBAC-.

#![allow(clippy::significant_drop_in_scrutinee)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

mod addr;
mod bitmap;
pub mod interface;
mod objects;
mod palette;
mod render;
pub mod scheduling;
pub mod threading;
mod tile;

use common::{
    numutil::{NumExt, U16Ext},
    Colour,
};

use crate::{
    addr::{
        BG0CNT, BG2PA, BG3PA, BLDALPHA, BLDCNT, BLDY, DISPCNT, DISPSTAT, VCOUNT, WIN0H, WIN0V,
        WININ, WINOUT,
    },
    interface::{PpuDmaReason, PpuInterrupt, PpuSystem},
    scheduling::PpuEvent,
    threading::PpuType,
};

const KB: usize = 1024;

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

// False positives since a constant of the bound is used
#[allow(type_alias_bounds)]
type Layer<S: PpuSystem> = [Colour; S::W];
#[allow(type_alias_bounds)]
type WindowMask<S: PpuSystem> = [bool; S::W];

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Ppu<S: PpuSystem>
where
    [(); S::W * S::H]:,
{
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub palette: [u8; KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub vram: [u8; 96 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub oam: [u8; KB],

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_colour_arr::<S>"))]
    pixels: [Colour; S::W * S::H],
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_layer_arr::<S>"))]
    bg_layers: [Layer<S>; 4],
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_layer_arr::<S>"))]
    bg_pixels: [Layer<S>; 4],
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_layer_arr::<S>"))]
    obj_layers: [Layer<S>; 4],

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_mask_arr::<S>"))]
    win_masks: [WindowMask<S>; 5],
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_mask2_arr::<S>"))]
    win_blend: [bool; S::W],

    bg_x: [i32; 2],
    bg_y: [i32; 2],

    #[cfg_attr(all(feature = "serde", not(feature = "threaded")), serde(default))]
    #[cfg_attr(all(feature = "serde", not(feature = "threaded")), serde(skip))]
    #[cfg(not(feature = "threaded"))]
    pub last_frame: Option<Vec<Colour>>,
}

impl<S: PpuSystem> Ppu<S>
where
    [(); S::W * S::H]:,
{
    pub fn handle_event(gg: &mut S, event: PpuEvent, late_by: i32) {
        let (next_event, cycles) = match event {
            PpuEvent::HblankStart => {
                Self::render_line_maybe_threaded(gg);
                Self::maybe_interrupt(gg, PpuInterrupt::HBlank, HBLANK_IRQ);
                gg.notify_dma(PpuDmaReason::HBlank);

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
                    Self::maybe_interrupt(gg, PpuInterrupt::VCounter, LYC_IRQ);
                } else {
                    gg[DISPSTAT] = gg[DISPSTAT].set_bit(LYC_MATCH, false);
                }

                let vcount = gg[VCOUNT].us();
                match () {
                    _ if vcount == S::H => {
                        gg[DISPSTAT] = gg[DISPSTAT].set_bit(VBLANK, true);
                        Self::maybe_interrupt(gg, PpuInterrupt::VBlank, VBLANK_IRQ);
                        gg.notify_dma(PpuDmaReason::VBlank);
                    }
                    // VBlank flag gets set one scanline early
                    _ if vcount == (S::VBLANK_END - 1) => {
                        gg[DISPSTAT] = gg[DISPSTAT].set_bit(VBLANK, false)
                    }
                    _ if vcount == S::VBLANK_END => {
                        gg[VCOUNT] = 0;
                        let frame = Self::end_frame(gg);
                        gg.ppu().last_frame = Some(frame);
                        gg.frame_finished();
                    }
                    _ => (),
                }

                gg[DISPSTAT] = gg[DISPSTAT].set_bit(HBLANK, false);
                (PpuEvent::HblankStart, 960)
            }
        };
        gg.schedule(next_event, cycles - late_by);
    }

    fn maybe_interrupt(gg: &mut S, int: PpuInterrupt, bit: u16) {
        if gg[DISPSTAT].is_bit(bit) {
            gg.request_interrupt(int);
        }
    }

    #[cfg(not(feature = "threaded"))]
    fn render_line_maybe_threaded(gg: &mut S) {
        let mmio = gg.ppu_mmio();
        Self::render_line(&mut PpuType {
            mmio,
            ppu: gg.ppu(),
        });
    }

    #[cfg(feature = "threaded")]
    fn render_line_maybe_threaded(gg: &mut S) {
        let mmio = gg.ppu_mmio();
        gg.ppu().thread.render(mmio);
    }

    fn render_line(gg: &mut PpuType<S>) {
        let line = gg[VCOUNT];
        if line >= S::H.u16() {
            return;
        }
        if gg[DISPCNT].is_bit(FORCED_BLANK) {
            let start = line.us() * S::W;
            for pixel in 0..S::W {
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
            inv => log::warn!("Invalid PPU mode {inv}"),
        }

        Self::finish_line(gg, line);
    }

    fn calc_windows(gg: &mut PpuType<S>, line: u8) {
        let cnt = gg[DISPCNT];
        let win0_en = cnt.is_bit(WIN0_EN);
        let win1_en = cnt.is_bit(WIN1_EN);
        let win_obj_en = cnt.is_bit(WIN_OBJS);
        if !win0_en && !win1_en && !win_obj_en {
            // No windows enabled, allow everything to draw
            gg.ppu.win_masks = serde_mask_arr::<S>();
            gg.ppu.win_blend = serde_mask2_arr::<S>();
            return;
        }

        let win = gg[WININ];
        let wout = gg[WINOUT];

        // First set parameters for the outside region, anything in windows
        // will be overwritten later.
        for mask in 0..5 {
            let enable = wout.is_bit(mask);
            gg.ppu.win_masks[mask.us()] = [enable; S::W];
        }
        let blend = wout.is_bit(5);
        gg.ppu.win_blend = [blend; S::W];

        // Do window 1 first, since it has lower priority and we want
        // window 0 to overwrite it
        for window in (0..2).rev() {
            if !cnt.is_bit(WIN0_EN + window.u16()) {
                // Window disabled.
                continue;
            }

            let win_offs = (window * 8).u16();
            let (x1, x2) = Self::window_coords(S::W as u8, gg[WIN0H + window * 2]);
            let (y1, y2) = Self::window_coords(S::H as u8, gg[WIN0V + window * 2]);

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
            let blend = win.is_bit(win_offs + 5);
            for pixel in x1..x2 {
                gg.ppu.win_blend[pixel.us()] = blend;
            }
        }
    }

    // Return WIN0/WIN1 coordinates, first is start, second is end.
    fn window_coords(max: u8, value: u16) -> (u8, u8) {
        let x1 = value.high();
        let x2 = value.low().saturating_add(1).min(max);
        if x1 > x2 {
            (x1, max)
        } else {
            (x1, x2)
        }
    }

    fn finish_line(gg: &mut PpuType<S>, line: u16) {
        let start = line.us() * S::W;
        let mut backdrop = gg.ppu.idx_to_palette::<false>(0); // BG0 is backdrop

        let bldcnt = gg[BLDCNT];
        let blend_mode = bldcnt.bits(6, 2);

        if blend_mode == 0 {
            // Fast path: No blending
            Self::adjust_pixel(&mut backdrop);

            #[cfg(not(feature = "threaded"))]
            let ppu = &mut gg.ppu;
            #[cfg(feature = "threaded")]
            let ppu = &mut *gg.ppu;

            'pixels: for (x, pixel) in ppu.pixels[start..].iter_mut().take(S::W).enumerate() {
                for prio in 0..4 {
                    if ppu.obj_layers[prio][x][3] != EMPTY_A {
                        *pixel = ppu.obj_layers[prio][x];
                        Self::adjust_pixel(pixel);
                        continue 'pixels;
                    }
                    if ppu.bg_layers[prio][x][3] != EMPTY_A {
                        *pixel = ppu.bg_layers[prio][x];
                        Self::adjust_pixel(pixel);
                        continue 'pixels;
                    }
                }

                // No matching pixel found...
                *pixel = backdrop;
            }
        } else {
            // Slow path: Blending
            let firsts = [
                bldcnt.is_bit(0),
                bldcnt.is_bit(1),
                bldcnt.is_bit(2),
                bldcnt.is_bit(3),
                bldcnt.is_bit(4),
                bldcnt.is_bit(5),
            ];
            let seconds = [
                bldcnt.is_bit(8),
                bldcnt.is_bit(9),
                bldcnt.is_bit(10),
                bldcnt.is_bit(11),
                bldcnt.is_bit(12),
                bldcnt.is_bit(13),
            ];
            let cnt = gg[DISPCNT];
            let mut bg_prio = [
                gg[BG0CNT] & 3,
                gg[BG0CNT + 2] & 3,
                gg[BG0CNT + 4] & 3,
                gg[BG0CNT + 6] & 3,
            ];
            for i in 0..4 {
                if !cnt.is_bit(BG0_EN + i) {
                    bg_prio[i.us()] = 42;
                }
            }

            let bld = if blend_mode == 1 {
                gg[BLDALPHA]
            } else {
                gg[BLDY]
            };

            #[cfg(not(feature = "threaded"))]
            let ppu = &mut gg.ppu;
            #[cfg(feature = "threaded")]
            let ppu = &mut *gg.ppu;

            'bldpixels: for (x, pixel) in ppu.pixels[start..].iter_mut().take(S::W).enumerate() {
                pixel[3] = EMPTY_A;
                let enabled = ppu.win_blend[x];

                let mut was_obj = false;
                for prio in 0..4 {
                    if ppu.obj_layers[prio][x][3] != EMPTY_A {
                        let done = Self::calc_pixel::<true>(
                            pixel,
                            enabled,
                            ppu.obj_layers[prio][x],
                            blend_mode,
                            firsts[4],
                            seconds[4],
                            was_obj,
                            bld,
                        );
                        was_obj = true;

                        if done {
                            continue 'bldpixels;
                        }
                    }

                    for bg in 0..4 {
                        if bg_prio[bg] == prio.u16() && ppu.bg_pixels[bg][x][3] != EMPTY_A {
                            let done = Self::calc_pixel::<true>(
                                pixel,
                                enabled,
                                ppu.bg_pixels[bg][x],
                                blend_mode,
                                firsts[bg],
                                seconds[bg],
                                was_obj,
                                bld,
                            );

                            if done {
                                continue 'bldpixels;
                            }
                        }
                    }
                }

                // No matching pixel found...
                Self::calc_pixel::<true>(
                    pixel, enabled, backdrop, blend_mode, firsts[5], seconds[5], was_obj, bld,
                );
            }

            for pixel in gg.ppu.pixels[start..].iter_mut().take(S::W) {
                Self::adjust_pixel(pixel);
            }
        }

        // Clear last line buffers
        gg.ppu.bg_layers = serde_layer_arr::<S>();
        gg.ppu.bg_pixels = serde_layer_arr::<S>();
        gg.ppu.obj_layers = serde_layer_arr::<S>();
    }

    #[allow(clippy::too_many_arguments)]
    fn calc_pixel<const OBJ: bool>(
        pixel: &mut Colour,
        enabled: bool,
        colour: Colour,
        blend_mode: u16,
        first: bool,
        second: bool,
        was_obj: bool,
        bld: u16,
    ) -> bool {
        match () {
            _ if !enabled => {
                // Blending disabled here, can be with windowing
                *pixel = colour;
                true
            }

            _ if (was_obj && second) || blend_mode == 1 => {
                // Regular alphablend.
                if pixel[3] == EMPTY_A {
                    *pixel = colour;
                    !first
                } else {
                    if second {
                        *pixel = Self::blend_pixel(*pixel, colour, bld.bits(0, 5), bld.bits(8, 5));
                    }
                    second
                }
            }

            _ => {
                // B/W alphablend.
                let second_colour = if blend_mode == 2 {
                    [31, 31, 31, 255]
                } else {
                    [0, 0, 0, 255]
                };
                let evy = (bld & 31).min(0x10);
                if was_obj {
                    *pixel = Self::blend_pixel(*pixel, second_colour, 0x10 - evy, evy);
                    return true;
                }

                if first {
                    *pixel = Self::blend_pixel(colour, second_colour, 0x10 - evy, evy);
                } else {
                    *pixel = colour;
                }
                !OBJ
            }
        }
    }

    fn blend_pixel(first: Colour, second: Colour, factor_a: u16, factor_b: u16) -> Colour {
        let fa = factor_a.min(0x10);
        let fb = factor_b.min(0x10);
        let r = (first[0].u16() * fa + second[0].u16() * fb) >> 4;
        let g = (first[1].u16() * fa + second[1].u16() * fb) >> 4;
        let b = (first[2].u16() * fa + second[2].u16() * fb) >> 4;
        [r.u8(), g.u8(), b.u8(), 255]
    }

    fn end_frame(gg: &mut S) -> Vec<Colour> {
        // Reload affine backgrounds
        let bg_x0 = Self::get_affine_offs(gg[BG2PA + 0x8], gg[BG2PA + 0xA]);
        let bg_y0 = Self::get_affine_offs(gg[BG2PA + 0xC], gg[BG2PA + 0xE]);
        let bg_x1 = Self::get_affine_offs(gg[BG3PA + 0x8], gg[BG3PA + 0xA]);
        let bg_y1 = Self::get_affine_offs(gg[BG3PA + 0xC], gg[BG3PA + 0xE]);

        let mut ppu = {
            #[cfg(feature = "threaded")]
            {
                gg.ppu().ppu.lock().unwrap()
            }
            #[cfg(not(feature = "threaded"))]
            gg.ppu()
        };
        ppu.bg_x[0] = bg_x0;
        ppu.bg_y[0] = bg_y0;
        ppu.bg_x[1] = bg_x1;
        ppu.bg_y[1] = bg_y1;

        ppu.pixels.to_vec()
    }

    #[inline]
    fn adjust_pixel(pixel: &mut Colour) {
        for col in pixel.iter_mut().take(3) {
            *col = (*col << 3) | (*col >> 2);
        }
    }
}

impl<S: PpuSystem> Default for Ppu<S>
where
    [(); S::W * S::H]:,
{
    fn default() -> Self {
        Self {
            palette: [0; KB],
            vram: [0; 96 * KB],
            oam: [0; KB],

            bg_x: [0; 2],
            bg_y: [0; 2],

            pixels: serde_colour_arr::<S>(),
            bg_layers: serde_layer_arr::<S>(),
            bg_pixels: serde_layer_arr::<S>(),
            obj_layers: serde_layer_arr::<S>(),
            win_masks: serde_mask_arr::<S>(),
            win_blend: serde_mask2_arr::<S>(),

            #[cfg(not(feature = "threaded"))]
            last_frame: None,
        }
    }
}

fn serde_colour_arr<S: PpuSystem>() -> [Colour; S::W * S::H] {
    [[0, 0, 0, 255]; S::W * S::H]
}
fn serde_layer_arr<S: PpuSystem>() -> [Layer<S>; 4]
where
    [(); S::W]:,
{
    [[EMPTY; S::W]; 4]
}
fn serde_mask_arr<S: PpuSystem>() -> [WindowMask<S>; 5]
where
    [(); S::W]:,
{
    [[true; S::W]; 5]
}
fn serde_mask2_arr<S: PpuSystem>() -> [bool; S::W] {
    [true; S::W]
}

const EMPTY: Colour = [0, 0, 0, 0];
const EMPTY_A: u8 = 0;
