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
// NDS9
pub const EXMEM: u32 = 0x204;
pub const VRAMCNT_A: u32 = 0x240;
pub const VRAMCNT_C: u32 = 0x242;
pub const VRAMCNT_E: u32 = 0x244;
pub const VRAMCNT_G: u32 = 0x246;
pub const VRAMCNT_H: u32 = 0x248;
// NDS7
pub const VRAMSTAT: u32 = 0x240;

// Graphics
pub const DISPCNT_L: u32 = 0x0;
pub const DISPCNT_H: u32 = 0x0;
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
pub const DISP3DCNT: u32 = 0x60;
pub const DISPCAPCNT_L: u32 = 0x64;
pub const DISPCAPCNT_H: u32 = 0x66;
pub const DISP_MMEM_FIFO_L: u32 = 0x68;
pub const DISP_MMEM_FIFO_H: u32 = 0x6A;
pub const MASTER_BRIGHT: u32 = 0x6C;

// Timers
pub const TM0CNT_L: u32 = 0x100;
pub const TM1CNT_L: u32 = 0x104;
pub const TM2CNT_L: u32 = 0x108;
pub const TM3CNT_L: u32 = 0x10C;
pub const TM0CNT_H: u32 = 0x102;
pub const TM1CNT_H: u32 = 0x106;
pub const TM2CNT_H: u32 = 0x10A;
pub const TM3CNT_H: u32 = 0x10E;

// Math
pub const DIVCNT_L: u32 = 0x280;
pub const DIVCNT_H: u32 = 0x282;
pub const DIV_NUMER: u32 = 0x290;
pub const DIV_DENOM: u32 = 0x298;
pub const DIV_RESULT: u32 = 0x2A0;
pub const DIV_REM: u32 = 0x2A8;
pub const SQRTCNT_L: u32 = 0x2B0;
pub const SQRTCNT_H: u32 = 0x2B2;
pub const SQRT_RESULT_L: u32 = 0x2B4;
pub const SQRT_RESULT_H: u32 = 0x2B6;
pub const SQRT_INPUT: u32 = 0x2B8;

// IPC FIFO
pub const IPCSYNC: u32 = 0x180;
pub const IPCFIFOCNT: u32 = 0x184;
pub const IPCFIFOSEND: u32 = 0x188;
pub const IPCFIFORECV: u32 = 0x100_0000;

// Input
pub const KEYINPUT: u32 = 0x130;
pub const KEYCNT: u32 = 0x130;
pub const EXTKEYIN: u32 = 0x136;
