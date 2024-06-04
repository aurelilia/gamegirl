// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{render::affine_transform_point, xy2d, OverflowMode, Ppu, HEIGHT, WIDTH};

impl Ppu {
    pub fn render_mode0(&mut self) {
        for bg in 0..4 {
            self.render_bg_text(bg);
        }
        self.finalize_scanline(0..=3);
    }

    pub fn render_mode1(&mut self) {
        self.render_bg_text(0);
        self.render_bg_text(1);
        self.render_bg_affine(2);
        self.finalize_scanline(0..=2);
    }

    pub fn render_mode2(&mut self) {
        self.render_bg_affine(2);
        self.render_bg_affine(3);
        self.finalize_scanline(2..=3);
    }

    pub fn render_mode3(&mut self) {
        if !self.bg_enabled(2) {
            return;
        }

        let wrap = self.bg_cnt[2].overflow_mode() == OverflowMode::Wraparound;
        for x in 0..WIDTH {
            let mut point = affine_transform_point(
                self.bg_scale[0].latched,
                x as i32,
                self.bg_scale[0].pa as i32,
                self.bg_scale[0].pc as i32,
            );

            if !point.inbounds(WIDTH, HEIGHT) {
                if wrap {
                    point.0 = point.0.rem_euclid(WIDTH as i32);
                    point.1 = point.1.rem_euclid(HEIGHT as i32);
                } else {
                    continue;
                }
            }

            let pixel = xy2d(point.0 as usize, point.1 as usize);
            self.bg_layers[2][x] = self.hword_to_colour_vram(pixel << 1);
        }

        self.finalize_scanline(2..=2);
    }

    pub fn render_mode4(&mut self) {
        if !self.bg_enabled(2) {
            return;
        }

        let start_addr = self.bitmap_start_addr();
        let wrap = self.bg_cnt[2].overflow_mode() == OverflowMode::Wraparound;
        for x in 0..WIDTH {
            let mut point = affine_transform_point(
                self.bg_scale[0].latched,
                x as i32,
                self.bg_scale[0].pa as i32,
                self.bg_scale[0].pc as i32,
            );

            if !point.inbounds(WIDTH, HEIGHT) {
                if wrap {
                    point.0 = point.0.rem_euclid(WIDTH as i32);
                    point.1 = point.1.rem_euclid(HEIGHT as i32);
                } else {
                    continue;
                }
            }

            let pixel = xy2d(point.0 as usize, point.1 as usize);
            let palette = self.vram[start_addr + pixel];
            if palette != 0 {
                self.bg_layers[2][x] = self.idx_to_palette::<false>(palette);
            }
        }

        self.finalize_scanline(2..=2);
    }

    pub fn render_mode5(&mut self) {
        if self.vcount > 127 || !self.bg_enabled(2) {
            return;
        }

        let wrap = self.bg_cnt[2].overflow_mode() == OverflowMode::Wraparound;
        for x in 0..WIDTH {
            let mut point = affine_transform_point(
                self.bg_scale[0].latched,
                x as i32,
                self.bg_scale[0].pa as i32,
                self.bg_scale[0].pc as i32,
            );

            if !point.inbounds(160, 127) {
                if wrap {
                    point.0 = point.0.rem_euclid(160 as i32);
                    point.1 = point.1.rem_euclid(127 as i32);
                } else {
                    continue;
                }
            }

            let pixel = ((point.1 as usize) * 160) + point.0 as usize;
            self.bg_layers[2][x] = self.hword_to_colour_vram(pixel << 1);
        }

        self.finalize_scanline(2..=2);
    }

    fn bitmap_start_addr(&self) -> usize {
        if self.dispcnt.frame_select() {
            0xA000
        } else {
            0x0
        }
    }
}
