// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{mem, ops::Range};

use common::{
    components::thin_pager::{ThinPager, RW},
    numutil::NumExt,
};
use modular_bitfield::{bitfield, specifiers::*};

use crate::memory::KB;

pub const A: usize = 0;
pub const B: usize = 1;
pub const C: usize = 2;
pub const D: usize = 3;
pub const E: usize = 4;
pub const F: usize = 5;
pub const G: usize = 6;
pub const H: usize = 7;
pub const I: usize = 8;
const EMPTY: usize = 9;

const LCDC_ADDRS: [u32; 9] = [
    0x80_0000, 0x82_0000, 0x84_0000, 0x86_0000, 0x88_0000, 0x89_0000, 0x89_4000, 0x89_8000,
    0x8A_0000,
];

#[bitfield]
#[repr(u8)]
#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct VramCtrl {
    mst: B3,
    ofs: B2,
    #[skip]
    __: B2,
    enable: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Vram {
    pub ctrls: [VramCtrl; 9],
    pub v: [Box<[u8]>; 9],
    pub pager: ThinPager,
}

impl Vram {
    pub fn vram_stat(&self) -> u8 {
        let c = self.ctrls[C].enable() && self.ctrls[C].mst() == 2;
        let d = self.ctrls[D].enable() && self.ctrls[D].mst() == 2;
        c as u8 | ((d as u8) << 1)
    }

    pub fn update_ctrl(&mut self, r: usize, new: u8, p7: &mut ThinPager, p9: &mut ThinPager) {
        let new: VramCtrl = new.into();
        let old = mem::replace(&mut self.ctrls[r], new);
        if new == old || (!new.enable() && !old.enable()) {
            return; // Nothing to be done
        }

        if old.enable() {
            let start = self.calc_range_for(r, old);
            if let Some(start) = start {
                p9.evict(self.get_range(start, r.u8()));
            }
        }
        if new.enable() {
            let start = self.calc_range_for(r, new);
            if let Some(start) = start {
                let range = self.get_range(start, r.u8());
                p9.map(&self.v[r], range, RW);
            }
        }

        if r == C || r == D {
            if old.enable() && old.mst() == 2 && old.ofs() < 2 {
                let start = 0x600_0000 + (0x2_0000 * old.ofs().u32());
                p7.evict(start..(start + 0x2_0000));
            }
            if new.enable() && new.mst() == 2 && new.ofs() < 2 {
                let start = 0x600_0000 + (0x2_0000 * new.ofs().u32());
                p7.map(&self.v[r], start..(start + 0x2_0000), RW);
            }
        }
    }

    pub fn init_mappings(&mut self, p7: &mut ThinPager, p9: &mut ThinPager) {
        self.pager = p9.clone();
        for i in 0..9 {
            let c = self.ctrls[i];
            if !c.enable() {
                continue;
            }

            let start = self.calc_range_for(i, c);
            if let Some(start) = start {
                let range = self.get_range(start, i.u8());
                p9.map(&self.v[i], range, RW);
            }
        }
    }

    fn calc_range_for(&self, r: usize, ctrl: VramCtrl) -> Option<u32> {
        let ofs = ctrl.ofs().u32();
        Some(match (r, ctrl.mst()) {
            // LCDC
            (_, 0) => LCDC_ADDRS[r],

            // BG A
            (A..=D, 1) => ofs * 0x2_0000,
            (E, 1) => 0,
            (F | G, 1) => (0x4000 * ofs.bit(0)) + (0x1_0000 * ofs.bit(1)),

            // OBJ A
            (A..=D, 2) => 0x40_0000 + (ofs.bit(0) * 0x2_0000),
            (E, 2) => 0x40_0000,
            (F | G, 1) => 0x40_0000 + (0x4000 * ofs.bit(0)) + (0x1_0000 * ofs.bit(1)),

            // EXTPAL A, Texture, Texture Palette
            // (unmapped for the CPU)

            // BG B
            (C, 4) | (H, 1) => 0x20_0000,
            (I, 1) => 0x20_8000,

            // OBJ B
            (D, 4) | (I, 2) => 0x60_0000,

            // EXTPAL B
            // (unmapped for the CPU)

            // Some kind of invalid mapping.
            _ => return None,
        })
    }

    fn get_range(&mut self, start: u32, size_of: u8) -> Range<u32> {
        let start = start + 0x600_0000;
        let end = start + self.v[size_of.us()].len().u32();
        debug_assert!(start & 0x3FFF == 0);
        debug_assert!(end & 0x3FFF == 0);
        start..end
    }

    pub fn get9(&self, addr: usize) -> Option<u8> {
        self.pager.read(0x600_0000 + addr.u32())
    }
}

impl Default for Vram {
    fn default() -> Self {
        Self {
            ctrls: [VramCtrl::default(); 9],
            v: [
                Box::new([0; 128 * KB]),
                Box::new([0; 128 * KB]),
                Box::new([0; 128 * KB]),
                Box::new([0; 128 * KB]),
                Box::new([0; 64 * KB]),
                Box::new([0; 16 * KB]),
                Box::new([0; 16 * KB]),
                Box::new([0; 32 * KB]),
                Box::new([0; 16 * KB]),
            ],
            pager: ThinPager::default(),
        }
    }
}
