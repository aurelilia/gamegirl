// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use super::{super::OverflowMode, pixels::affine_transform_point, xy2d, PpuRender, HEIGHT, WIDTH};

impl PpuRender {
    pub fn render_mode0(&mut self) {
        for bg in 0..4 {
            self.render_bg_text(bg);
        }
        self.finalize_scanline(0..=3);
    }

    pub fn render_mode1(&mut self) {
        self.render_bg_text(0);
        self.render_bg_text(1);
        self.render_bg_text(2);
        self.render_bg_affine(3);
        self.finalize_scanline(0..=3);
    }

    pub fn render_mode2(&mut self) {
        self.render_bg_text(0);
        self.render_bg_text(1);
        self.render_bg_affine(2);
        self.render_bg_affine(3);
        self.finalize_scanline(0..=3);
    }

    pub fn render_mode3(&mut self) {
        self.render_bg_text(0);
        self.render_bg_text(1);
        self.render_bg_text(2);
        self.render_bg_ext(3);
        self.finalize_scanline(0..=3);
    }

    pub fn render_mode4(&mut self) {
        self.render_bg_text(0);
        self.render_bg_text(1);
        self.render_bg_affine(2);
        self.render_bg_ext(3);
        self.finalize_scanline(0..=3);
    }

    pub fn render_mode5(&mut self) {
        self.render_bg_text(0);
        self.render_bg_text(1);
        self.render_bg_ext(2);
        self.render_bg_ext(3);
        self.finalize_scanline(0..=3);
    }

    pub fn render_mode6(&mut self) {
        // TODO 3D stuff
    }
}
