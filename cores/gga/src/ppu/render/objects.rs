// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::cmp;

use common::{
    numutil::{hword, NumExt},
    Colour,
};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use super::{
    super::{registers::PaletteMode, Point},
    xy2dw, PpuRender, HEIGHT, TRANS, WIDTH,
};
use crate::ppu::CharacterMappingMode;

const OBJ_X_SIZE: [u16; 16] = [8, 16, 32, 64, 16, 32, 32, 64, 8, 8, 16, 32, 8, 8, 8, 8];
const OBJ_Y_SIZE: [u16; 16] = [8, 16, 32, 64, 8, 8, 16, 32, 16, 32, 32, 64, 8, 8, 8, 8];

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ObjPixel {
    pub colour: Colour,
    pub is_window: bool,
    pub is_alpha: bool,
    pub priority: u8,
}

impl Default for ObjPixel {
    fn default() -> Self {
        Self {
            colour: TRANS,
            is_window: false,
            is_alpha: false,
            priority: 255,
        }
    }
}

impl PpuRender {
    pub(super) fn render_objs(&mut self) {
        for idx in 0..128 {
            let obj = self.get_object(idx);
            match obj.kind() {
                ObjectKind::Normal => self.render_obj_normal(obj),
                ObjectKind::Affine => self.render_obj_affine(obj, false),
                ObjectKind::AffineDouble => self.render_obj_affine(obj, true),
                ObjectKind::Disable => (),
            }
        }
    }

    pub fn get_object(&self, idx: u8) -> Object {
        let addr = idx.us() << 3;
        let bytes = &self.oam[addr..(addr + 8)];
        Object::from_bytes(bytes.try_into().unwrap())
    }

    fn render_obj_normal(&mut self, obj: Object) {
        let line = self.r.vcount as i32;
        let position = obj.position();
        let (width, height) = obj.size();

        if !obj.draw_on(self.r.vcount, position.1, height as u8) {
            return; // Not on this line or invalid
        }

        let base_addr = 0x1_0000 + (0x20 * obj.tilenum().u32());
        if self.r.is_bitmap_mode() && obj.tilenum() < 512 {
            return; // Invalid tile number for bitmap mode(s)
        }
        let tile_width = obj.tile_width(self.r.dispcnt.character_mapping_mode(), width);
        let tile_size = obj.tile_size();

        let sprite_y = line - position.1;
        let sprite_y = if obj.rotscal().is_bit(4) {
            height as i32 - sprite_y - 1
        } else {
            sprite_y
        };
        let sprite_y = Self::maybe_mosaic(sprite_y, obj.mosaic_en(), self.r.mosaic.obj_v());

        let start = cmp::max(position.0, 0);
        let end = cmp::min(position.0 + width as i32, WIDTH as i32);
        for screen_x in start..end {
            let current = self.obj_pixel(screen_x as usize);
            if current.priority <= obj.priority() && obj.mode() != ObjectMode::ObjWindow {
                continue;
            }

            let sprite_x = screen_x - position.0;
            let sprite_x = if obj.rotscal().is_bit(3) {
                width as i32 - sprite_x - 1
            } else {
                sprite_x
            };
            let sprite_x = Self::maybe_mosaic(sprite_x, obj.mosaic_en(), self.r.mosaic.obj_h());

            let tile_addr = base_addr
                + xy2dw(
                    (sprite_x / 8) as usize,
                    (sprite_y / 8) as usize,
                    tile_width.us(),
                ) as u32
                    * tile_size;
            let palette = self.get_palette(
                obj.palette(),
                obj.palette_mode(),
                tile_addr,
                sprite_x as u32 % 8,
                sprite_y as u32 % 8,
            );

            if let Some(pal) = palette {
                let colour = self.idx_to_palette::<true>(pal);
                self.write_obj_pixel(screen_x as usize, colour, obj);
            }
        }
    }

    fn render_obj_affine(&mut self, obj: Object, double: bool) {
        let line = self.r.vcount as i32;
        let position = obj.position();
        let (width, height) = obj.size();
        let (width, height) = (width as i32, height as i32);
        let (bounds_w, bounds_h) = if double {
            (width * 2, height * 2)
        } else {
            (width, height)
        };

        if !obj.draw_on(self.r.vcount, position.1, bounds_h as u8) {
            return; // Not on this line or invalid
        }

        let base_addr = 0x1_0000 + (0x20 * obj.tilenum().u32());
        let tile_width = obj.tile_width(self.r.dispcnt.character_mapping_mode(), width as u16);
        let tile_size = obj.tile_size();

        let rotscal = self.get_rotscal(obj.rotscal());
        let (half_w, half_h) = (bounds_w / 2, bounds_h / 2);
        let iy = line - (position.1 + half_h as i32);

        for ix in (-half_w)..half_w {
            let screen_x = position.0 + half_w + ix;
            if screen_x < 0 {
                continue;
            }
            if screen_x >= WIDTH as i32 {
                break;
            }

            let current = self.obj_pixel(screen_x as usize);
            if current.priority <= obj.priority() && obj.mode() != ObjectMode::ObjWindow {
                continue;
            }

            let trans_x = ((rotscal[0] * ix) + (rotscal[1] * iy)) >> 8;
            let trans_y = ((rotscal[2] * ix) + (rotscal[3] * iy)) >> 8;
            let tex_x = trans_x + width / 2;
            let tex_y = trans_y + height / 2;

            if tex_x >= 0 && tex_x < width && tex_y >= 0 && tex_y < height {
                let tex_x = Self::maybe_mosaic(tex_x, obj.mosaic_en(), self.r.mosaic.obj_h());
                let tex_y = Self::maybe_mosaic(tex_y, obj.mosaic_en(), self.r.mosaic.obj_v());

                let tile_addr = base_addr
                    + xy2dw(
                        (tex_x / 8) as usize,
                        (tex_y / 8) as usize,
                        tile_width as usize,
                    ) as u32
                        * tile_size;
                let palette = self.get_palette(
                    obj.palette(),
                    obj.palette_mode(),
                    tile_addr,
                    tex_x as u32 % 8,
                    tex_y as u32 % 8,
                );

                if let Some(pal) = palette {
                    let colour = self.idx_to_palette::<true>(pal);
                    self.write_obj_pixel(screen_x as usize, colour, obj);
                }
            }
        }
    }

    /// Get the Rot/Scal parameters at the given OAM index.
    fn get_rotscal(&self, idx: u8) -> [i32; 4] {
        let mut offs = 32 * idx.us() + 6;
        let mut out = [0; 4];
        for elem in &mut out {
            *elem = hword(self.oam[offs], self.oam[offs + 1]) as i16 as i32;
            offs += 8;
        }
        out
    }

    pub(super) fn obj_pixel(&self, x: usize) -> ObjPixel {
        self.obj_layer[x]
    }

    fn write_obj_pixel(&mut self, x: usize, colour: Colour, obj: Object) {
        let pixel = &mut self.obj_layer[x];
        match obj.mode() {
            ObjectMode::Normal | ObjectMode::SemiTransparent => {
                pixel.colour = colour;
                pixel.priority = obj.priority();
                pixel.is_alpha = obj.mode() == ObjectMode::SemiTransparent;
            }
            ObjectMode::ObjWindow => pixel.is_window = true,
            ObjectMode::Prohibited => unreachable!(),
        }
    }
}

#[bitfield]
#[repr(u64)]
#[derive(Debug, Copy, Clone)]
pub struct Object {
    pub y: B8,
    pub kind: ObjectKind,
    pub mode: ObjectMode,
    pub mosaic_en: bool,
    pub palette_mode: PaletteMode,
    pub shape: B2,
    pub x: B9,
    pub rotscal: B5,
    pub obj_size: B2,
    pub tilenum: B10,
    pub priority: B2,
    pub palette: B4,
    #[skip]
    __: B16,
}

impl Object {
    pub fn size(self) -> (u16, u16) {
        let addr = (self.obj_size() | (self.shape() << 2)).us();
        (OBJ_X_SIZE[addr], OBJ_Y_SIZE[addr])
    }

    pub fn position(&self) -> Point {
        let mut y = self.y() as i16 as i32;
        let mut x = self.x() as i16 as i32;
        if y >= (HEIGHT as i32) {
            y -= 1 << 8;
        }
        if x >= (WIDTH as i32) {
            x -= 1 << 9;
        }
        Point(x, y)
    }

    pub fn tile_width(&self, mode: CharacterMappingMode, width: u16) -> u16 {
        match mode {
            CharacterMappingMode::TwoDim if self.palette_mode() == PaletteMode::Single256 => 16,
            CharacterMappingMode::TwoDim => 32,
            CharacterMappingMode::OneDim => width / 8,
        }
    }

    pub fn tile_size(&self) -> u32 {
        match self.palette_mode() {
            PaletteMode::Palettes16 => 0x20,
            PaletteMode::Single256 => 0x40,
        }
    }

    fn draw_on(self, line: u16, self_y: i32, size_y: u8) -> bool {
        self.valid()
            && (line as i32 >= self_y)
            && ((line as i32) < ((self_y).wrapping_add(size_y as i32)))
    }

    fn valid(self) -> bool {
        self.mode() != ObjectMode::Prohibited && self.shape() != 3
    }
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
pub enum ObjectMode {
    Normal = 0,
    SemiTransparent = 1,
    ObjWindow = 2,
    Prohibited = 3,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
pub enum ObjectKind {
    Normal = 0,
    Affine = 1,
    Disable = 2,
    AffineDouble = 3,
}
