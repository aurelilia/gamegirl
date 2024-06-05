// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

// Interrupts
pub const IME: u32 = 0x208;
pub const IE_L: u32 = 0x210;
pub const IE_H: u32 = 0x212;
pub const IF_L: u32 = 0x214;
pub const IF_H: u32 = 0x216;

// Memory control
pub const EXMEM: u32 = 0x204;
pub const WRAMCNT: u32 = 0x246;
pub const VRAMCNTSTAT: u32 = 0x240;

// PPU
pub const DISPCNT: u32 = 0x0;
pub const DISPSTAT: u32 = 0x4;
pub const VCOUNT: u32 = 0x6;

// Timers
pub const TM0CNT_L: u32 = 0x100;
pub const TM1CNT_L: u32 = 0x104;
pub const TM2CNT_L: u32 = 0x108;
pub const TM3CNT_L: u32 = 0x10C;
pub const TM0CNT_H: u32 = 0x102;
pub const TM1CNT_H: u32 = 0x106;
pub const TM2CNT_H: u32 = 0x10A;
pub const TM3CNT_H: u32 = 0x10E;
