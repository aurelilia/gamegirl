// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{
    numutil::{hword, NumExt},
    Colour,
};

use super::{xy2dw, PaletteMode, Ppu};

impl Ppu {
    /// Given a tile address and tile pixel, get the palette.
    pub(super) fn get_palette(
        &self,
        bank: u8,
        mode: PaletteMode,
        addr: u32,
        x: u32,
        y: u32,
    ) -> Option<u8> {
        Some(match mode {
            PaletteMode::Palettes16 => {
                let addr = addr.us() + xy2dw(x.us() / 2, y.us(), 4);
                if addr >= self.vram.len() {
                    return None;
                }
                let value = self.vram[addr];
                let colour = if x.is_bit(0) { value >> 4 } else { value & 0xF };
                if colour == 0 {
                    return None;
                }
                (bank * 0x10) + colour
            }
            PaletteMode::Single256 => {
                let addr = addr.us() + xy2dw(x.us(), y.us(), 8);
                if addr >= self.vram.len() {
                    return None;
                }
                let pal = self.vram[addr];
                if pal == 0 {
                    return None;
                }
                pal
            }
        })
    }

    /// Turn a halfword in VRAM into a 5bit colour.
    pub fn hword_to_colour_vram(&self, addr: usize) -> Colour {
        let lo = self.vram[addr];
        let hi = self.vram[addr + 1];
        Self::hword_to_colour(hword(lo, hi))
    }

    /// Turn a palette index (0-255) into a colour.
    pub fn idx_to_palette<const OBJ: bool>(&self, idx: u8) -> Colour {
        let addr = (idx.us() << 1) + (OBJ as usize * 0x200);
        let lo = self.palette[addr];
        let hi = self.palette[addr + 1];
        Self::hword_to_colour(hword(lo, hi))
    }

    /// Extract a 5bit colour from a halfword.
    fn hword_to_colour(hword: u16) -> Colour {
        let r = hword.bits(0, 5).u8();
        let g = hword.bits(5, 5).u8();
        let b = hword.bits(10, 5).u8();
        [r, g, b, 255]
    }
}
