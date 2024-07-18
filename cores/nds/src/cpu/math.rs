// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::{hword, word, NumExt, U16Ext, U32Ext};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Div {
    pub ctrl: DivControl,
    pub numer: u64,
    pub denom: u64,
    pub result: u64,
    pub rem: u64,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DivControl {
    mode: DivMode,
    #[skip]
    __: B12,
    by_zero: bool,
    busy: bool,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DivMode {
    All32 = 0,
    Partial1 = 1,
    Partial2 = 3,
    All64 = 2,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Sqrt {
    pub ctrl: SqrtControl,
    pub input: u64,
    pub result: u32,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SqrtControl {
    mode: SqrtMode,
    #[skip]
    __: B14,
    busy: bool,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum SqrtMode {
    Bit32 = 0,
    Bit64 = 1,
}
