// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::numutil::{hword, word, NumExt, U16Ext};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use super::{Point, Ppu, WIDTH};
use crate::addr::*;

impl Ppu {
    pub(super) fn bg_enabled(&self, bg: u16) -> bool {
        self.dispcnt.bg_en().is_bit(bg)
    }

    pub fn read_mmio(&self, addr: u32) -> Option<u16> {
        Some(match addr {
            DISPCNT => self.dispcnt.into(),
            GREENSWAP => self.greepswap.into(),
            DISPSTAT => self.dispstat.into(),
            VCOUNT => self.vcount,

            BG0CNT => self.bg_cnt[0].into(),
            BG1CNT => self.bg_cnt[1].into(),
            BG2CNT => self.bg_cnt[2].into(),
            BG3CNT => self.bg_cnt[3].into(),

            WININ => hword(
                self.windows[0].control.into(),
                self.windows[1].control.into(),
            ),
            WINOUT => hword(self.win_out.into(), self.win_obj.into()),
            MOSAIC => self.mosaic.into(),
            BLDCNT => self.bldcnt.into(),
            BLDALPHA => self.bldalpha.into(),

            _ => return None,
        })
    }

    pub fn write_mmio(&mut self, addr: u32, value: u16) {
        match addr {
            DISPCNT => self.dispcnt = value.into(),
            GREENSWAP => self.greepswap = value.into(),
            DISPSTAT => {
                let disp: u16 = self.dispstat.into();
                self.dispstat = ((disp & 0b111) | (value & !0b1100_0111)).into();
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
                0x8 => self.bg_scale[addr.bit(4).us()].xl = value,
                0xA => self.bg_scale[addr.bit(4).us()].xh = value,
                0xC => self.bg_scale[addr.bit(4).us()].yl = value,
                0xE => self.bg_scale[addr.bit(4).us()].yh = value,
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
                self.windows[0].control = value.low().into();
                self.windows[1].control = value.high().into();
            }
            WINOUT => {
                self.win_out = value.low().into();
                self.win_obj = value.high().into();
            }
            MOSAIC => self.mosaic = value.into(),

            BLDCNT => self.bldcnt = (value & 0x3FFF).into(),
            BLDALPHA => self.bldalpha = (value & 0x1F1F).into(),
            BLDY => self.bldy = value & 0x1F,

            _ => (),
        }
    }
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DisplayControl {
    pub bg_mode: BackgroundMode,
    #[skip]
    reserved_cgb: bool,
    pub frame_select: bool,
    pub hblank_oam_free: bool,
    pub character_mapping_mode: CharacterMappingMode,
    pub forced_blank_enable: bool,
    pub bg_en: B4,
    pub obj_en: bool,
    pub win0_en: bool,
    pub win1_en: bool,
    pub winobj_en: bool,
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
    ProhibitedA = 6,
    ProhibitedB = 7,
}

#[derive(BitfieldSpecifier, Debug)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CharacterMappingMode {
    TwoDim = 0,
    OneDim = 1,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct GreenSwap {
    pub green_swap_en: bool,
    #[skip]
    __: B15,
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
        fn get_affine_offs(lo: u16, hi: u16) -> i32 {
            if hi.is_bit(11) {
                (word(lo, hi & 0x7FF) | 0xF800_0000) as i32
            } else {
                word(lo, hi & 0x7FF) as i32
            }
        }

        self.latched = Point(
            get_affine_offs(self.xl, self.xh),
            get_affine_offs(self.yl, self.yh),
        );
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
    pub bg_mosaic_h: B4,
    pub bg_mosaic_v: B4,
    pub obj_mosaic_h: B4,
    pub obj_mosaic_v: B4,
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