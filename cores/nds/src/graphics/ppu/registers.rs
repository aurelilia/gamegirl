// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::cmp;

use common::numutil::{hword, word, NumExt, U16Ext, U32Ext};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use super::{Point, WIDTH};
use crate::addr::*;

#[derive(Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PpuRegisters {
    pub is_a: bool,
    pub(super) vcount: u16,

    pub dispcnt: DisplayControl,
    pub bg_cnt: [BgControl; 4],
    pub bg_offsets: [u16; 8],
    pub bg_scale: [BgRotScal; 2],

    pub windows: [Window; 2],
    pub win_obj: WindowCtrl,
    pub win_out: WindowCtrl,

    pub(super) mosaic: Mosaic,
    pub(super) bldcnt: BlendControl,
    pub(super) bldalpha: BlendAlpha,
    pub(super) bldy: u16,
}

impl PpuRegisters {
    pub(super) fn bg_enabled(&self, bg: u16) -> bool {
        self.dispcnt.bg_en().is_bit(bg)
    }

    pub fn is_bitmap_mode(&self) -> bool {
        self.dispcnt.bg_mode() as usize >= 3
    }

    pub fn read_mmio(&self, addr: u32) -> Option<u16> {
        Some(match addr {
            DISPCNT_L => u32::from(self.dispcnt).low(),
            DISPCNT_H => u32::from(self.dispcnt).high(),

            BG0CNT => self.bg_cnt[0].into(),
            BG1CNT => self.bg_cnt[1].into(),
            BG2CNT => self.bg_cnt[2].into(),
            BG3CNT => self.bg_cnt[3].into(),

            WININ => hword(
                self.windows[0].control.into(),
                self.windows[1].control.into(),
            ),
            WINOUT => hword(self.win_out.into(), self.win_obj.into()),
            BLDCNT => self.bldcnt.into(),
            BLDALPHA => self.bldalpha.into(),

            _ => return None,
        })
    }

    pub fn get_mmio_inner(&mut self, addr: u32) -> u16 {
        match addr {
            BG0HOFS..=BG3VOFS => self.bg_offsets[(addr.us() & 0xF) >> 1],
            BG2PA..WIN0H => match addr & 0xF {
                0x0 => self.bg_scale[addr.bit(4).us()].pa as u16,
                0x2 => self.bg_scale[addr.bit(4).us()].pb as u16,
                0x4 => self.bg_scale[addr.bit(4).us()].pc as u16,
                0x6 => self.bg_scale[addr.bit(4).us()].pd as u16,
                0x8 => self.bg_scale[addr.bit(4).us()].xl,
                0xA => self.bg_scale[addr.bit(4).us()].xh,
                0xC => self.bg_scale[addr.bit(4).us()].yl,
                0xE => self.bg_scale[addr.bit(4).us()].yh,
                _ => unreachable!(),
            },

            WIN0H => hword(self.windows[0].right, self.windows[0].left),
            WIN1H => hword(self.windows[1].right, self.windows[1].left),
            WIN0V => hword(self.windows[0].bottom, self.windows[0].top),
            WIN1V => hword(self.windows[1].bottom, self.windows[1].top),

            MOSAIC => self.mosaic.into(),
            BLDY => self.bldy,

            _ => self.read_mmio(addr).unwrap_or(0),
        }
    }

    pub fn write_mmio_byte(&mut self, addr_unalign: u32, value: u8) {
        let addr = addr_unalign & !1;
        let var = self.get_mmio_inner(addr);
        if addr_unalign.is_bit(0) {
            self.write_mmio(addr, var.set_high(value));
        } else {
            self.write_mmio(addr, var.set_low(value));
        }
    }

    pub fn write_mmio(&mut self, addr: u32, value: u16) {
        match addr {
            DISPCNT_L => {
                self.dispcnt = (u32::from(self.dispcnt) & 0xFFFF_0000 | value.u32()).into()
            }
            DISPCNT_H => {
                self.dispcnt = (u32::from(self.dispcnt) & 0x0000_FFFF | (value.u32() << 16)).into()
            }

            BG0CNT => self.bg_cnt[0] = (value & 0xDFFF).into(),
            BG1CNT => self.bg_cnt[1] = (value & 0xDFFF).into(),
            BG2CNT => self.bg_cnt[2] = value.into(),
            BG3CNT => self.bg_cnt[3] = value.into(),

            BG0HOFS..=BG3VOFS => self.bg_offsets[(addr.us() & 0xF) >> 1] = value & 0x1FF,

            BG2PA..WIN0H => match addr & 0xF {
                0x0 => self.bg_scale[addr.bit(4).us()].pa = value as i16,
                0x2 => self.bg_scale[addr.bit(4).us()].pb = value as i16,
                0x4 => self.bg_scale[addr.bit(4).us()].pc = value as i16,
                0x6 => self.bg_scale[addr.bit(4).us()].pd = value as i16,
                0x8 => {
                    self.bg_scale[addr.bit(4).us()].xl = value;
                    self.bg_scale[addr.bit(4).us()].latch_x();
                }
                0xA => {
                    self.bg_scale[addr.bit(4).us()].xh = value;
                    self.bg_scale[addr.bit(4).us()].latch_x();
                }
                0xC => {
                    self.bg_scale[addr.bit(4).us()].yl = value;
                    self.bg_scale[addr.bit(4).us()].latch_y();
                }
                0xE => {
                    self.bg_scale[addr.bit(4).us()].yh = value;
                    self.bg_scale[addr.bit(4).us()].latch_y();
                }
                _ => (),
            },

            WIN0H => {
                self.windows[0].right = value.low();
                self.windows[0].left = value.high();
            }
            WIN1H => {
                self.windows[1].right = value.low();
                self.windows[1].left = value.high();
            }
            WIN0V => {
                self.windows[0].bottom = value.low();
                self.windows[0].top = value.high();
            }
            WIN1V => {
                self.windows[1].bottom = value.low();
                self.windows[1].top = value.high();
            }
            WININ => {
                self.windows[0].control = (value.low() & 0x3F).into();
                self.windows[1].control = (value.high() & 0x3F).into();
            }
            WINOUT => {
                self.win_out = (value.low() & 0x3F).into();
                self.win_obj = (value.high() & 0x3F).into();
            }
            MOSAIC => self.mosaic = value.into(),

            BLDCNT => self.bldcnt = (value & 0x3FFF).into(),
            BLDALPHA => self.bldalpha = (value & 0x1F1F).into(),
            BLDY => self.bldy = cmp::min(16, value & 0x1F),

            _ => (),
        }
    }
}

#[bitfield]
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DisplayControl {
    pub bg_mode: BackgroundMode,
    pub bg0_is_3d: bool,
    pub tile_obj_mode: CharacterMappingMode,
    pub bitmap_obj_256dot: bool,
    pub bitmap_obj_mode: CharacterMappingMode,
    pub forced_blank_enable: bool,
    pub bg_en: B4,
    pub obj_en: bool,
    pub win0_en: bool,
    pub win1_en: bool,
    pub winobj_en: bool,

    pub display_mode: DisplayMode,
    pub vram_block: B2,
    pub tile_obj_1d_boundary: B2,
    pub bitmap_obj_1d_boundary: B1,
    pub hblank_oam_free: bool,
    pub character_base_block: B3,
    pub screen_base_block: B3,
    pub bg_ext_pal_enable: bool,
    pub obj_ext_pal_enable: bool,
}

impl DisplayControl {
    #[inline]
    pub(super) fn win_enabled(&self) -> bool {
        self.win0_en() || self.win1_en() || self.winobj_en()
    }
}

#[derive(BitfieldSpecifier, Debug, Copy, Clone)]
#[bits = 3]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum BackgroundMode {
    Mode0 = 0,
    Mode1 = 1,
    Mode2 = 2,
    Mode3 = 3,
    Mode4 = 4,
    Mode5 = 5,
    Mode6 = 6,
    ProhibitedB = 7,
}

#[derive(BitfieldSpecifier, Debug, Copy, Clone)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DisplayMode {
    DisplayOff = 0,
    Normal = 1,
    VramDisplay = 2,
    MemoryDisplay = 3,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DisplayStatus {
    pub in_vblank: bool,
    pub in_hblank: bool,
    pub vcounter_match: bool,
    pub irq_enables: B3,
    #[skip]
    __: B2,
    pub vcount: u8,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BgControl {
    pub priority: B2,
    pub character_base_block: B2,
    #[skip]
    __: B2,
    pub mosaic_en: bool,
    pub palette_mode: PaletteMode,
    pub screen_base_block: B5,
    pub overflow_mode: OverflowMode,
    pub screen_size: B2,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PaletteMode {
    Palettes16 = 0,
    Single256 = 1,
}

#[derive(BitfieldSpecifier, Debug)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CharacterMappingMode {
    TwoDim = 0,
    OneDim = 1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum OverflowMode {
    Transparent = 0,
    Wraparound = 1,
}

#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BgRotScal {
    pub pa: i16,
    pub pb: i16,
    pub pc: i16,
    pub pd: i16,
    pub xl: u16,
    pub xh: u16,
    pub yl: u16,
    pub yh: u16,
    pub latched: Point,
}

impl BgRotScal {
    pub fn latch(&mut self) {
        self.latched = Point(
            Self::get_affine_offs(self.xl, self.xh),
            Self::get_affine_offs(self.yl, self.yh),
        );
    }

    pub fn latch_x(&mut self) {
        self.latched.0 = Self::get_affine_offs(self.xl, self.xh);
    }

    pub fn latch_y(&mut self) {
        self.latched.1 = Self::get_affine_offs(self.yl, self.yh);
    }

    fn get_affine_offs(lo: u16, hi: u16) -> i32 {
        if hi.is_bit(11) {
            (word(lo, hi & 0x7FF) | 0xF800_0000) as i32
        } else {
            word(lo, hi & 0x7FF) as i32
        }
    }
}

#[bitfield]
#[repr(u8)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct WindowCtrl {
    pub bg_en: B4,
    pub obj_en: bool,
    pub special_en: bool,
    #[skip]
    __: B2,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Window {
    pub left: u8,
    pub right: u8,
    pub top: u8,
    pub bottom: u8,
    pub control: WindowCtrl,
}

impl Window {
    #[inline]
    pub fn left(&self) -> usize {
        self.left as usize
    }

    #[inline]
    pub fn right(&self) -> usize {
        let left = self.left as usize;
        let mut right = self.right as usize;
        if right > WIDTH || right < left {
            right = WIDTH;
        }
        right
    }

    #[inline]
    pub fn top(&self) -> usize {
        self.top as usize
    }

    #[inline]
    pub fn bottom(&self) -> usize {
        let top = self.top as usize;
        let mut bottom = self.bottom as usize;
        if bottom > WIDTH || bottom < top {
            bottom = WIDTH;
        }
        bottom
    }

    #[inline]
    pub fn contains_y(&self, y: usize) -> bool {
        let top = self.top();
        let bottom = self.bottom();
        y >= top && y < bottom
    }
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Mosaic {
    pub bg_h: B4,
    pub bg_v: B4,
    pub obj_h: B4,
    pub obj_v: B4,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BlendControl {
    pub first_target: B6,
    pub special_effect: SpecialEffect,
    pub second_target: B6,
    #[skip]
    __: B2,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum SpecialEffect {
    None = 0,
    AlphaBlend = 1,
    BrightnessInc = 2,
    BrightnessDec = 3,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BlendAlpha {
    pub eva: B5,
    #[skip]
    __: B3,
    pub evb: B5,
    #[skip]
    __: B3,
}
