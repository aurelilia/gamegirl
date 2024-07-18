// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{mem, ops::Range};

use common::numutil::NumExt;
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

const LCDC_ADDRS: [usize; 9] = [
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
    pub arm9_map: Box<[(u8, u8)]>,
}

impl Vram {
    pub fn vram_stat(&self) -> u8 {
        let c = self.ctrls[C].enable() && self.ctrls[C].mst() == 2;
        let d = self.ctrls[D].enable() && self.ctrls[D].mst() == 2;
        c as u8 | ((d as u8) << 1)
    }

    pub fn update_ctrl(&mut self, r: usize, new: u8) {
        let new: VramCtrl = new.into();
        let old = mem::replace(&mut self.ctrls[r], new);
        if new == old || (!new.enable() && !old.enable()) {
            return; // Nothing to be done
        }

        if old.enable() {
            let start = self.calc_range_for(r, old);
            if let Some(start) = start {
                self.set_mapping_range(start, EMPTY.u8(), r.u8())
            }
        }
        if new.enable() {
            let start = self.calc_range_for(r, new);
            if let Some(start) = start {
                self.set_mapping_range(start, r.u8(), r.u8())
            }
        }
    }

    fn calc_range_for(&self, r: usize, ctrl: VramCtrl) -> Option<usize> {
        let ofs = ctrl.ofs().us();
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

    fn get_mapping(&self, addr: usize) -> (u8, usize) {
        let (mapping, offset) = self.arm9_map[(addr & 0xFF_FFFF) >> 14];
        (mapping, (offset.us() * 0x4000) + (addr & 0x3FFF))
    }

    fn set_mapping_range(&mut self, start: usize, mapping: u8, size_of: u8) {
        let end = start + self.v[size_of.us()].len();
        debug_assert!(start & 0x3FFF == 0);
        debug_assert!(start & 0x3FFF == 0);

        let start = start >> 14;
        let end = end >> 14;
        for (offs, entry) in self.arm9_map[start..end].iter_mut().enumerate() {
            *entry = (mapping, offs.u8());
        }
    }

    pub fn get7(&self, addr: usize) -> Option<&[u8]> {
        let a = addr & 0x3_FFFF;
        let ofs = (a >= 0x2_000) as u8;
        for i in C..=D {
            if self.is_mst_ofs(i, 2, ofs) {
                return Some(&self.v[i]);
            }
        }
        None
    }

    pub fn get7_mut(&mut self, addr: usize) -> Option<&mut [u8]> {
        let a = addr & 0x3_FFFF;
        let ofs = (a >= 0x2_000) as u8;
        for i in C..=D {
            if self.is_mst_ofs(i, 2, ofs) {
                return Some(&mut self.v[i]);
            }
        }
        None
    }

    pub fn get9(&self, addr: usize) -> Option<&[u8]> {
        let (mapping, offs) = self.get_mapping(addr);
        self.v.get(mapping.us()).map(|v| &v[offs..])
    }

    pub fn get9_mut(&mut self, addr: usize) -> Option<&mut [u8]> {
        let (mapping, offs) = self.get_mapping(addr);
        self.v.get_mut(mapping.us()).map(|v| &mut v[offs..])
    }

    fn is_mst(&self, ctrl: usize, mst: u8) -> bool {
        let c = self.ctrls[ctrl];
        c.enable() && c.mst() == mst
    }

    fn is_mst_ofs(&self, ctrl: usize, mst: u8, ofs: u8) -> bool {
        let c = self.ctrls[ctrl];
        c.enable() && c.mst() == mst && c.ofs() == ofs
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
            arm9_map: Box::new([(0, EMPTY as u8); 1024]),
        }
    }
}
