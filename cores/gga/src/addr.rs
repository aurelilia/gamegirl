// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

// General
pub const IE: u32 = 0x200;
pub const IF: u32 = 0x202;
pub const WAITCNT: u32 = 0x204;
pub const IME: u32 = 0x208;
pub const POSTFLG: u32 = 0x300;

// PPU
pub const DISPCNT: u32 = 0x0;
pub const GREENSWAP: u32 = 0x2;
pub const DISPSTAT: u32 = 0x4;
pub const VCOUNT: u32 = 0x6;
pub const BG0CNT: u32 = 0x8;
pub const BG1CNT: u32 = 0xA;
pub const BG2CNT: u32 = 0xC;
pub const BG3CNT: u32 = 0xE;
pub const BG0HOFS: u32 = 0x10;
pub const BG0VOFS: u32 = 0x12;
pub const BG3VOFS: u32 = 0x1E;
pub const BG2PA: u32 = 0x20;
pub const BG3PA: u32 = 0x30;
pub const WIN0H: u32 = 0x40;
pub const WIN1H: u32 = 0x42;
pub const WIN0V: u32 = 0x44;
pub const WIN1V: u32 = 0x46;
pub const WININ: u32 = 0x48;
pub const WINOUT: u32 = 0x4A;
pub const MOSAIC: u32 = 0x4C;
pub const BLDCNT: u32 = 0x50;
pub const BLDALPHA: u32 = 0x52;
pub const BLDY: u32 = 0x54;

// Input
pub const KEYINPUT: u32 = 0x130;
pub const KEYCNT: u32 = 0x132;

// Timers
pub const TM0CNT_L: u32 = 0x100;
pub const TM1CNT_L: u32 = 0x104;
pub const TM2CNT_L: u32 = 0x108;
pub const TM3CNT_L: u32 = 0x10C;
pub const TM0CNT_H: u32 = 0x102;
pub const TM1CNT_H: u32 = 0x106;
pub const TM2CNT_H: u32 = 0x10A;
pub const TM3CNT_H: u32 = 0x10E;

// Audio
pub const SOUNDCNT_H: u32 = 0x82;
pub const SOUNDBIAS_L: u32 = 0x88;
pub const FIFO_A_L: u32 = 0xA0;
pub const FIFO_A_H: u32 = 0xA2;
pub const FIFO_B_L: u32 = 0xA4;
pub const FIFO_B_H: u32 = 0xA6;

// Serial
pub const SIOCNT: u32 = 0x128;
