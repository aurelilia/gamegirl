// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::cmp;

use common::{
    components::io::IoSection,
    io16, io32, iow08, iow16, iow32,
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use super::{Point, WIDTH};
use crate::addr::*;

#[derive(Default, Debug, Clone)]
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

    pub fn read(&self, a: u32) -> (u32, u32, u32) {
        io32!(a, DISPCNT_L, self.dispcnt.into());

        io16!(a, BG0CNT, self.bg_cnt[0].into());
        io16!(a, BG1CNT, self.bg_cnt[1].into());
        io16!(a, BG2CNT, self.bg_cnt[2].into());
        io16!(a, BG3CNT, self.bg_cnt[3].into());

        io16!(
            a,
            WININ,
            hword(
                self.windows[0].control.into(),
                self.windows[1].control.into(),
            )
        );
        io16!(a, WINOUT, hword(self.win_out.into(), self.win_obj.into()));
        io16!(a, BLDCNT, self.bldcnt.into());
        io16!(a, BLDALPHA, self.bldalpha.into());

        log::error!("Read from unknown PPU IO register 0x{a:X}");
        (0, 0, 1)
    }

    pub fn write(
        &mut self,
        a: u32,
        s8: IoSection<u8>,
        s16: IoSection<u16>,
        s32: IoSection<u32>,
    ) -> (u32, u32) {
        iow32!(a, DISPCNT_L, s32.apply_io(&mut self.dispcnt));

        iow16!(a, BG0CNT, s16.mask(0xDFFF).apply_io(&mut self.bg_cnt[0]));
        iow16!(a, BG1CNT, s16.mask(0xDFFF).apply_io(&mut self.bg_cnt[1]));
        iow16!(a, BG2CNT, s16.apply_io(&mut self.bg_cnt[2]));
        iow16!(a, BG3CNT, s16.apply_io(&mut self.bg_cnt[3]));

        if matches!(a, 0x10..=0x1F) {
            s16.mask(0x1FF)
                .apply(&mut self.bg_offsets[(a.us() & 0xF) >> 1]);
            return (a & 1, 2);
        }

        for i in 0..2 {
            let offs = i * 0x10;
            let scale = &mut self.bg_scale[i.us()];
            iow16!(a, BG2PA + offs, scale.pa = s16.with(scale.pa as u16) as i16);
            iow16!(a, BG2PB + offs, scale.pb = s16.with(scale.pb as u16) as i16);
            iow16!(a, BG2PC + offs, scale.pc = s16.with(scale.pc as u16) as i16);
            iow16!(a, BG2PD + offs, scale.pd = s16.with(scale.pd as u16) as i16);
            iow16!(a, BG2XL + offs, {
                s16.apply(&mut scale.xl);
                scale.latch_x();
            });
            iow16!(a, BG2XH + offs, {
                s16.apply(&mut scale.xh);
                scale.latch_x();
            });
            iow16!(a, BG2YL + offs, {
                s16.apply(&mut scale.yl);
                scale.latch_y();
            });
            iow16!(a, BG2YH + offs, {
                s16.apply(&mut scale.yh);
                scale.latch_y();
            });
        }

        iow08!(a, WIN0H, s8.apply(&mut self.windows[0].right));
        iow08!(a, WIN0H + 1, s8.apply(&mut self.windows[0].left));
        iow08!(a, WIN0V, s8.apply(&mut self.windows[0].bottom));
        iow08!(a, WIN0V + 1, s8.apply(&mut self.windows[0].top));
        iow08!(a, WIN1H, s8.apply(&mut self.windows[1].right));
        iow08!(a, WIN1H + 1, s8.apply(&mut self.windows[1].left));
        iow08!(a, WIN1V, s8.apply(&mut self.windows[1].bottom));
        iow08!(a, WIN1V + 1, s8.apply(&mut self.windows[1].top));

        iow08!(
            a,
            WININ,
            s8.mask(0x3F).apply_io(&mut self.windows[0].control)
        );
        iow08!(
            a,
            WININ + 1,
            s8.mask(0x3F).apply_io(&mut self.windows[1].control)
        );
        iow08!(a, WINOUT, s8.mask(0x3F).apply_io(&mut self.win_out));
        iow08!(a, WINOUT + 1, s8.mask(0x3F).apply_io(&mut self.win_obj));
        iow16!(a, MOSAIC, s16.apply_io(&mut self.mosaic));

        iow16!(a, BLDCNT, s16.mask(0x3FFF).apply_io(&mut self.bldcnt));
        iow16!(a, BLDALPHA, s16.mask(0x1F1F).apply_io(&mut self.bldalpha));
        iow08!(a, BLDY, self.bldy = cmp::min(16, s8.raw() & 0x1F).u16());

        log::error!(
            "Write to unknown PPU IO register 0x{a:X} with 0x{:X}",
            s32.with(0)
        );
        (0, 1)
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
    pub character_base_block: B4,
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

#[derive(Debug, Copy, Clone)]
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

impl Default for BgRotScal {
    fn default() -> Self {
        Self {
            pa: 256,
            pb: 0,
            pc: 0,
            pd: 256,
            xl: 0,
            xh: 0,
            yl: 0,
            yh: 0,
            latched: Point::default(),
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
