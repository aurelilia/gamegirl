// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::num;

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

impl Div {
    pub fn update(&mut self) {
        self.ctrl.set_by_zero(self.denom == 0);
        match self.ctrl.mode() {
            DivMode::All32 if self.numer.u32() == 0x80000000 && self.denom.u32() == u32::MAX => {
                self.result = 0x80000000;
                self.rem = 0;
            }
            DivMode::All32 if self.denom.u32() == 0 => {
                if (self.numer as i32) < 0 {
                    self.result = 1 | ((u32::MAX as u64) << 32);
                    self.rem = self.numer | ((u32::MAX as u64) << 32);
                } else {
                    self.result = u32::MAX as u64;
                    self.rem = self.numer;
                }
            }
            DivMode::All32 => {
                self.result =
                    (self.numer.u32() as i32).wrapping_div(self.denom.u32() as i32) as i64 as u64;
                self.rem = (self.numer.u32() as i32)
                    .checked_rem(self.denom.u32() as i32)
                    .unwrap_or(0) as i64 as u64;
            }

            DivMode::Partial1 if self.denom.u32() == 0 => {
                if (self.numer as i64) < 0 {
                    self.result = 1;
                } else {
                    self.result = u64::MAX;
                }
                self.rem = self.numer;
            }
            DivMode::Partial1 => {
                self.result =
                    (self.numer as i64).wrapping_div(self.denom.u32() as i32 as i64) as u64;
                self.rem = (self.numer as i64)
                    .checked_rem(self.denom.u32() as i32 as i64)
                    .unwrap_or(0) as i64 as u64;
            }

            DivMode::All64 | DivMode::Reserved if self.denom == 0 => {
                if (self.numer as i64) < 0 {
                    self.result = 1;
                } else {
                    self.result = u64::MAX;
                }
                self.rem = self.numer;
            }
            DivMode::All64 | DivMode::Reserved => {
                self.result = (self.numer as i64).wrapping_div(self.denom as i64) as u64;
                self.rem = (self.numer as i64)
                    .checked_rem(self.denom as i64)
                    .unwrap_or(0) as u64;
            }
        }
    }
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
    Reserved = 3,
    All64 = 2,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Sqrt {
    pub ctrl: SqrtControl,
    pub input: u64,
    pub result: u32,
}

impl Sqrt {
    pub fn update(&mut self) {
        match self.ctrl.mode() {
            SqrtMode::Bit32 => self.result = self.input.u32().isqrt(),
            SqrtMode::Bit64 => self.result = self.input.isqrt() as u32,
        }
    }
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
