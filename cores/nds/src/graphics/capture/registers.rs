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
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DispCapCnt {
    pub eva: B5,
    #[skip]
    __: B3,
    pub evb: B5,
    #[skip]
    __: B3,
    vram_write_block: B2,
    vram_write_offs: B2,
    capt_size: CaptureSize,
    #[skip]
    __: B2,
    source_a: SourceA,
    source_b: SourceB,
    vram_read_offs: B2,
    #[skip]
    __: B1,
    capt_source: CaptureSource,
    capt_en: bool,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CaptureSize {
    C128x128 = 0,
    C256x64 = 1,
    C256x128 = 2,
    C256x192 = 3,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum SourceA {
    Graphics = 0,
    ThreeD = 1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum SourceB {
    Vram = 0,
    MemFifo = 1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CaptureSource {
    SrcA = 0,
    SrcB = 1,
    SrcAB = 2,
    SrcBA = 3,
}
