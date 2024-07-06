// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{cmp, mem, ops::RangeInclusive};

use arrayvec::ArrayVec;
use common::{numutil::NumExt, Colour};

use super::{super::Point, xy2d, PpuRender, WIDTH};
use crate::ppu::{SpecialEffect, WindowCtrl, BLACK, WHITE};

fn blend(a: Colour, b: Colour, a_weight: u16, b_weight: u16) -> Colour {
    let r = cmp::min(31, (a[0].u16() * a_weight + b[0].u16() * b_weight) >> 4).u8();
    let g = cmp::min(31, (a[1].u16() * a_weight + b[1].u16() * b_weight) >> 4).u8();
    let b = cmp::min(31, (a[2].u16() * a_weight + b[2].u16() * b_weight) >> 4).u8();
    [r, g, b, 255]
}

fn filter_active_window_bgs(backgrounds: &[usize], winctrl: WindowCtrl) -> ArrayVec<usize, 4> {
    backgrounds
        .iter()
        .copied()
        .filter(|bg| winctrl.bg_en().is_bit(bg.u16()))
        .collect()
}

pub(super) fn affine_transform_point(ref_point: Point, screen_x: i32, pa: i32, pc: i32) -> Point {
    let Point(ref_x, ref_y) = ref_point;
    Point((ref_x + screen_x * pa) >> 8, (ref_y + screen_x * pc) >> 8)
}

impl PpuRender {
    /// Composes the render layers into a final scanline while applying needed
    /// special effects, and render it to the frame buffer
    pub(super) fn finalize_scanline(&mut self, bgs: RangeInclusive<usize>) {
        self.compose_scanline(bgs.clone());
        self.clean_buffers(bgs);
    }

    fn compose_scanline(&mut self, bgs: RangeInclusive<usize>) {
        let backdrop_color = self.idx_to_palette::<false>(0);

        // filter out disabled backgrounds and sort by priority
        // the backgrounds are sorted once for the entire scanline
        let mut sorted_backgrounds: ArrayVec<usize, 4> = bgs
            .clone()
            .filter(|bg| self.r.bg_enabled(bg.u16()))
            .collect();
        sorted_backgrounds.sort_by_key(|bg| (self.r.bg_cnt[*bg].priority(), *bg));

        let y = self.r.vcount.us();
        if !self.r.dispcnt.win_enabled() {
            for x in 0..WIDTH {
                let win = WindowInfo::new(WindowCtrl::from_bytes([255]));
                self.finalize_pixel(x, y, &win, &sorted_backgrounds, backdrop_color);
            }
        } else {
            let mut occupied = [false; WIDTH];
            let mut occupied_count = 0;
            if self.r.dispcnt.win0_en() && self.r.windows[0].contains_y(y) {
                let win = WindowInfo::new(self.r.windows[0].control);
                let backgrounds = filter_active_window_bgs(&sorted_backgrounds, win.ctrl);
                for (x, is_occupied) in occupied
                    .iter_mut()
                    .enumerate()
                    .take(self.r.windows[0].right())
                    .skip(self.r.windows[0].left())
                {
                    self.finalize_pixel(x, y, &win, &backgrounds, backdrop_color);
                    *is_occupied = true;
                    occupied_count += 1;
                }
            }
            if occupied_count == WIDTH {
                return;
            }
            if self.r.dispcnt.win1_en() && self.r.windows[1].contains_y(y) {
                let win = WindowInfo::new(self.r.windows[1].control);
                let backgrounds = filter_active_window_bgs(&sorted_backgrounds, win.ctrl);
                for (x, is_occupied) in occupied
                    .iter_mut()
                    .enumerate()
                    .take(self.r.windows[1].right())
                    .skip(self.r.windows[1].left())
                    .filter(|(_, o)| !**o)
                {
                    self.finalize_pixel(x, y, &win, &backgrounds, backdrop_color);
                    *is_occupied = true;
                    occupied_count += 1;
                }
            }
            if occupied_count == WIDTH {
                return;
            }
            let win_out = WindowInfo::new(self.r.win_out);
            let win_out_backgrounds = filter_active_window_bgs(&sorted_backgrounds, win_out.ctrl);
            if self.r.dispcnt.winobj_en() {
                let win_obj = WindowInfo::new(self.r.win_obj);
                let win_obj_backgrounds =
                    filter_active_window_bgs(&sorted_backgrounds, win_obj.ctrl);
                for (x, _) in occupied.iter().enumerate().take(WIDTH).filter(|(_, o)| !*o) {
                    let obj_entry = self.obj_pixel(x);
                    if obj_entry.is_window {
                        // WinObj
                        self.finalize_pixel(x, y, &win_obj, &win_obj_backgrounds, backdrop_color);
                    } else {
                        // WinOut
                        self.finalize_pixel(x, y, &win_out, &win_out_backgrounds, backdrop_color);
                    }
                }
            } else {
                for x in 0..WIDTH {
                    if occupied[x] {
                        continue;
                    }
                    self.finalize_pixel(x, y, &win_out, &win_out_backgrounds, backdrop_color);
                }
            }
        }

        // Consider green swap
        if self.r.greepswap.green_swap_en() {
            for x in (0..WIDTH).step_by(2) {
                let a = self.pixels[xy2d(x, y)][1];
                let b = self.pixels[xy2d(x + 1, y)][1];
                self.pixels[xy2d(x, y)][1] = b;
                self.pixels[xy2d(x + 1, y)][1] = a;
            }
        }

        // Consider forced blank
        if self.r.dispcnt.forced_blank_enable() {
            for x in 0..WIDTH {
                self.pixels[xy2d(x, y)] = [255; 4];
            }
        }
    }

    fn finalize_pixel(
        &mut self,
        x: usize,
        y: usize,
        win: &WindowInfo,
        backgrounds: &[usize],
        backdrop_color: Colour,
    ) {
        // The backdrop layer is the default
        let backdrop_layer = RenderLayer::backdrop(backdrop_color);

        // Backgrounds are already sorted
        // lets start by taking the first 2 backgrounds that have an opaque pixel at x
        let mut it = backgrounds
            .iter()
            .filter(|i| self.bg_layers[**i][x][3] != 0)
            .take(2);
        let mut top_layer = it.next().map_or(backdrop_layer, |bg| {
            RenderLayer::background(*bg, self.bg_layers[*bg][x], self.r.bg_cnt[*bg].priority())
        });
        let mut bot_layer = it.next().map_or(backdrop_layer, |bg| {
            RenderLayer::background(*bg, self.bg_layers[*bg][x], self.r.bg_cnt[*bg].priority())
        });
        drop(it);

        // Now that backgrounds are taken care of, we need to check if there is an
        // object pixel that takes priority of one of the layers
        let obj_entry = self.obj_pixel(x);

        if win.ctrl.obj_en() && obj_entry.colour[3] != 0 {
            let obj_layer = RenderLayer::objects(obj_entry.colour, obj_entry.priority);
            if obj_layer.priority <= top_layer.priority {
                bot_layer = top_layer;
                top_layer = obj_layer;
            } else if obj_layer.priority <= bot_layer.priority {
                bot_layer = obj_layer;
            }
        }

        let obj_alpha_blend = top_layer.is_object() && obj_entry.is_alpha;
        let top_flags = self.r.bldcnt.first_target();
        let bot_flags = self.r.bldcnt.second_target();

        let sfx_enabled = (self.r.bldcnt.special_effect() != SpecialEffect::None
            || obj_alpha_blend)
            && (top_flags & top_layer.kind as u8) != 0; // sfx must at least have a first target configured

        let mut pixel = if win.ctrl.special_en() && sfx_enabled {
            if obj_alpha_blend && (bot_flags & bot_layer.kind as u8) != 0 {
                self.do_alpha(top_layer.pixel, bot_layer.pixel)
            } else {
                let (top_layer, bot_layer) = (top_layer, bot_layer);

                match self.r.bldcnt.special_effect() {
                    // Only alphablend with a second target.
                    SpecialEffect::AlphaBlend if (bot_flags & bot_layer.kind as u8) != 0 => {
                        self.do_alpha(top_layer.pixel, bot_layer.pixel)
                    }
                    SpecialEffect::BrightnessInc => self.do_brighten(top_layer.pixel),
                    SpecialEffect::BrightnessDec => self.do_darken(top_layer.pixel),
                    _ => top_layer.pixel,
                }
            }
        } else {
            // no blending, just use the top pixel
            top_layer.pixel
        };

        for col in pixel.iter_mut().take(3) {
            *col = (*col << 3) | (*col >> 2);
        }
        self.pixels[xy2d(x, y)] = pixel;
    }

    #[inline]
    fn do_alpha(&self, upper: Colour, lower: Colour) -> Colour {
        let eva = self.r.bldalpha.eva().into();
        let evb = self.r.bldalpha.evb().into();
        blend(upper, lower, eva, evb)
    }

    #[inline]
    fn do_brighten(&self, c: Colour) -> Colour {
        let evy = self.r.bldy;
        blend(c, WHITE, 16 - evy, evy)
    }

    #[inline]
    fn do_darken(&self, c: Colour) -> Colour {
        let evy = self.r.bldy;
        blend(c, BLACK, 16 - evy, evy)
    }

    pub(super) fn maybe_mosaic(val: i32, en: bool, mosaic: u8) -> i32 {
        if en {
            (val - (val % (mosaic as i32 + 1))).max(0)
        } else {
            val
        }
    }
}

#[derive(Debug)]
pub struct WindowInfo {
    pub ctrl: WindowCtrl,
}

impl WindowInfo {
    pub fn new(ctrl: WindowCtrl) -> WindowInfo {
        WindowInfo { ctrl }
    }
}

#[allow(unused)]
#[derive(Debug, Ord, Eq, PartialOrd, PartialEq, Clone, Copy)]
pub enum RenderLayerKind {
    Backdrop = 0b00100000,
    Background3 = 0b00001000,
    Background2 = 0b00000100,
    Background1 = 0b00000010,
    Background0 = 0b00000001,
    Objects = 0b00010000,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RenderLayer {
    pub kind: RenderLayerKind,
    pub priority: u8,
    pub pixel: Colour,
}

impl RenderLayer {
    pub fn background(bg: usize, pixel: Colour, priority: u8) -> RenderLayer {
        RenderLayer {
            kind: unsafe { mem::transmute(1u8 << bg) },
            pixel,
            priority,
        }
    }

    pub fn objects(pixel: Colour, priority: u8) -> RenderLayer {
        RenderLayer {
            kind: RenderLayerKind::Objects,
            pixel,
            priority,
        }
    }

    pub fn backdrop(pixel: Colour) -> RenderLayer {
        RenderLayer {
            kind: RenderLayerKind::Backdrop,
            pixel,
            priority: 4,
        }
    }

    pub(super) fn is_object(&self) -> bool {
        self.kind == RenderLayerKind::Objects
    }
}
