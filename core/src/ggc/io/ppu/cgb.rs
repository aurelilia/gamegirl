// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::{
    ggc::{
        io::{
            addr::*,
            ppu::{Ppu, PpuKind, BG_EN},
        },
        GameGirl,
    },
    numutil::NumExt,
    Colour,
};

/// Data required for a CGB PPU, mainly palette data.
#[derive(Deserialize, Serialize)]
pub struct Cgb {
    bg_palette_idx: u8,
    bg_palette_inc: bool,
    bg_palettes: [CgbColour; 32],
    obj_palette_idx: u8,
    obj_palette_inc: bool,
    pub(super) obj_palettes: [CgbColour; 32],

    pub(super) colour_correction: bool,
    #[serde(skip)]
    #[serde(default)]
    pub(super) dmg_used_x_obj_cords: Option<[Option<u8>; 10]>,
    #[serde(skip)]
    #[serde(default = "super::serde_bool_arr")]
    pub unavailable_pixels: [bool; 160],
}

impl Cgb {
    pub fn new(colour_correction: bool) -> Self {
        Self {
            bg_palette_idx: 0,
            bg_palette_inc: false,
            bg_palettes: [CgbColour::default(); 32],
            obj_palette_idx: 0,
            obj_palette_inc: false,
            obj_palettes: [CgbColour::default(); 32],
            colour_correction,
            dmg_used_x_obj_cords: None,
            unavailable_pixels: [false; 160],
        }
    }
}

/// A CGB palette colour.
#[derive(Copy, Clone, Default, Deserialize, Serialize)]
pub struct CgbColour {
    pub colour: Colour,
    raw_high: u8,
    raw_low: u8,
}

impl CgbColour {
    fn recalculate(&mut self, colour_correction: bool) {
        self.colour[0] = self.raw_low & 0x1F;
        self.colour[1] = ((self.raw_high & 3) << 3) | self.raw_low >> 5;
        self.colour[2] = (self.raw_high >> 2) & 0x1F;
        self.colour[3] = 255;

        if colour_correction {
            // https://near.sh/articles/video/color-emulation
            let (r, g, b) = (
                self.colour[0].u16(),
                self.colour[1].u16(),
                self.colour[2].u16(),
            );
            let r1 = r * 26 + g * 4 + b * 2;
            let g1 = g * 24 + b * 8;
            let b1 = r * 6 + g * 4 + b * 22;
            self.colour[0] = (r1.min(960) >> 2).u8();
            self.colour[1] = (g1.min(960) >> 2).u8();
            self.colour[2] = (b1.min(960) >> 2).u8();
        } else {
            for col in self.colour.iter_mut().take(3) {
                *col = (*col << 3) | (*col >> 2);
            }
        }
    }
}

impl Ppu {
    pub fn read_high(&self, addr: u16) -> u8 {
        match (&self.kind, addr) {
            (PpuKind::Cgb(cgb), BCPS) => Self::read_cps(cgb.bg_palette_idx, cgb.bg_palette_inc),
            (PpuKind::Cgb(cgb), OCPS) => Self::read_cps(cgb.obj_palette_idx, cgb.obj_palette_inc),
            (PpuKind::Cgb(cgb), BCPD) => Self::read_cpd(cgb.bg_palette_idx, &cgb.bg_palettes),
            (PpuKind::Cgb(cgb), OCPD) => Self::read_cpd(cgb.obj_palette_idx, &cgb.obj_palettes),
            _ => 0xFF,
        }
    }

    fn read_cps(index: u8, inc: bool) -> u8 {
        index & ((inc as u8) << 7)
    }

    fn read_cpd(index: u8, palettes: &[CgbColour]) -> u8 {
        let pal = palettes[(index.us() >> 1) & 0x1F];
        if index.is_bit(0) {
            pal.raw_high
        } else {
            pal.raw_low
        }
    }

    pub fn write_high(&mut self, addr: u16, value: u8) {
        match (&mut self.kind, addr) {
            (PpuKind::Cgb(cgb), BCPS) => {
                cgb.bg_palette_idx = value & 0x3F;
                cgb.bg_palette_inc = value.is_bit(7);
            }
            (PpuKind::Cgb(cgb), OCPS) => {
                cgb.obj_palette_idx = value & 0x3F;
                cgb.obj_palette_inc = value.is_bit(7);
            }
            (PpuKind::Cgb(cgb), BCPD) => Self::write_cpd(
                &mut cgb.bg_palette_idx,
                cgb.bg_palette_inc,
                &mut cgb.bg_palettes,
                cgb.colour_correction,
                value,
            ),
            (PpuKind::Cgb(cgb), OCPD) => Self::write_cpd(
                &mut cgb.obj_palette_idx,
                cgb.obj_palette_inc,
                &mut cgb.obj_palettes,
                cgb.colour_correction,
                value,
            ),
            (PpuKind::Cgb(cgb), OPRI) if value.is_bit(0) => {
                cgb.dmg_used_x_obj_cords = Some([None; 10])
            }
            (PpuKind::Cgb(cgb), OPRI) => cgb.dmg_used_x_obj_cords = None,
            _ => (),
        }
    }

    fn write_cpd(
        index: &mut u8,
        inc: bool,
        palettes: &mut [CgbColour],
        colour_correction: bool,
        value: u8,
    ) {
        let palette = &mut palettes[(index.us() >> 1) & 0x1F];
        if index.is_bit(0) {
            palette.raw_high = value;
        } else {
            palette.raw_low = value;
        };
        palette.recalculate(colour_correction);
        if inc {
            *index += 1;
        }
    }

    pub fn cgb_render_bg_or_window(
        gg: &mut GameGirl,
        scroll_x: u8,
        start_x: u8,
        end_x: u8,
        map_addr: u16,
        map_line: u8,
        correct_tile_addr: bool,
    ) {
        let line = gg.ppu.line;
        let bg_en = gg.lcdc(BG_EN);

        let mut tile_x = scroll_x & 7;
        let mut tile_addr = map_addr + ((map_line / 8).u16() * 0x20) + (scroll_x >> 3).u16();
        let mut attributes = gg.mem.vram[0x2000 + (tile_addr.us() & 0x1FFF)];
        let mut has_prio = attributes.is_bit(7) && bg_en;
        let mut tile_y = if attributes.is_bit(6) {
            7 - (map_line & 7)
        } else {
            map_line & 7
        };
        let mut tile_data_addr = Self::bg_tile_data_addr(gg, gg.mem.vram[tile_addr.us()])
            + (tile_y.u16() * 2)
            + attributes.bit(3).u16() * 0x2000;
        let mut high = gg.mem.vram[tile_data_addr.us() + 1];
        let mut low = gg.mem.vram[tile_data_addr.us()];

        for tile_idx_addr in start_x..end_x {
            let x = if attributes.is_bit(5) {
                tile_x
            } else {
                7 - tile_x
            }
            .u16();
            let colour_idx = (high.bit(x) << 1) + low.bit(x);
            gg.ppu().bg_occupied_pixels[tile_idx_addr.us()] |= (colour_idx != 0) && bg_en;

            let palette = attributes & 7;
            let colour = {
                let cgb = gg.cgb();
                cgb.unavailable_pixels[tile_idx_addr.us()] = (colour_idx != 0) && has_prio;
                cgb.bg_palettes[(palette.us() * 4) + colour_idx.us()]
            };
            gg.ppu().set_pixel(tile_idx_addr, line, colour.colour);

            tile_x += 1;
            if tile_x == 8 {
                tile_x = 0;
                tile_addr = if correct_tile_addr && (tile_addr & 0x1F) == 0x1F {
                    tile_addr - 0x1F
                } else {
                    tile_addr + 1
                };
                attributes = gg.mem.vram[0x2000 + (tile_addr.us() & 0x1FFF)];
                has_prio = attributes.is_bit(7) && bg_en;
                tile_y = if attributes.is_bit(6) {
                    7 - (map_line & 7)
                } else {
                    map_line & 7
                };
                tile_data_addr = Self::bg_tile_data_addr(gg, gg.mem.vram[tile_addr.us()])
                    + (tile_y.u16() * 2)
                    + attributes.bit(3).u16() * 0x2000;
                high = gg.mem.vram[tile_data_addr.us() + 1];
                low = gg.mem.vram[tile_data_addr.us()];
            }
        }
    }
}

impl GameGirl {
    fn cgb(&mut self) -> &mut Cgb {
        if let PpuKind::Cgb(cgb) = &mut self.ppu.kind {
            cgb
        } else {
            panic!()
        }
    }
}
