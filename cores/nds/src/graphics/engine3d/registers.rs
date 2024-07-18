// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::{hword, word, NumExt, U16Ext, U32Ext};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Disp3dCnt {
    pub tex_mapping_en: bool,
    pub shading: PolygonShading,
    pub alpha_test: bool,
    pub alpha_blend: bool,
    pub antialias: bool,
    pub edge_mark: bool,
    pub fog_color: FogColor,
    pub fog_en: bool,
    pub fog_shift: B4,
    pub color_underflow_ack: bool,
    pub polygon_overflow_ack: bool,
    pub rear_plane: RearPlane,
    #[skip]
    __: B1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PolygonShading {
    Toon = 0,
    Highlight = 1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum FogColor {
    AlphaColor = 0,
    Alpha = 1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RearPlane {
    Blank = 0,
    Bitmap = 1,
}
