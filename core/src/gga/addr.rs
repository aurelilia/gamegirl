// General
pub const IE: u32 = 0x200;
pub const IF: u32 = 0x202;
pub const WAITCNT: u32 = 0x204;
pub const IME: u32 = 0x208;
pub const HALTCNT: u32 = 0x300;

// PPU
pub const DISPCNT: u32 = 0x0;
pub const GREENSWAP: u32 = 0x2;
pub const DISPSTAT: u32 = 0x4;
pub const VCOUNT: u32 = 0x6;

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
