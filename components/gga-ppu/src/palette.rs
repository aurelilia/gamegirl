// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{
    numutil::{hword, NumExt},
    Colour,
};

use crate::{Ppu, PpuSystem};

impl<S: PpuSystem> Ppu<S>
where
    [(); S::W * S::H]:,
{
    pub fn hword_to_colour_vram(&self, addr: usize) -> Colour {
        let lo = self.vram[addr];
        let hi = self.vram[addr + 1];
        Self::hword_to_colour(hword(lo, hi))
    }

    pub fn idx_to_palette<const OBJ: bool>(&self, idx: u8) -> Colour {
        let addr = (idx.us() << 1) + (OBJ as usize * 0x200);
        let lo = self.palette[addr];
        let hi = self.palette[addr + 1];
        Self::hword_to_colour(hword(lo, hi))
    }

    fn hword_to_colour(hword: u16) -> Colour {
        let r = hword.bits(0, 5).u8();
        let g = hword.bits(5, 5).u8();
        let b = hword.bits(10, 5).u8();
        [r, g, b, 255]
    }
}
