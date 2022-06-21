use crate::{
    gga::{
        addr::*,
        cpu::Cpu,
        dma::Dmas,
        timer::Timers,
        Access::{self, *},
        GameGirlAdv,
    },
    ggc::cpu::Reg,
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};
use serde::{Deserialize, Serialize};
use std::{
    mem,
    ops::{Index, IndexMut},
};

pub const KB: usize = 1024;
pub const BIOS: &[u8] = include_bytes!("bios.bin");

#[derive(Deserialize, Serialize)]
pub struct Memory {
    #[serde(with = "serde_arrays")]
    pub ewram: [u8; 256 * KB],
    #[serde(with = "serde_arrays")]
    pub iwram: [u8; 32 * KB],
    #[serde(with = "serde_arrays")]
    pub mmio: [u16; KB / 2],
}

impl GameGirlAdv {
    /// Read a byte from the bus. Also enforces timing.
    pub(super) fn read_byte(&mut self, addr: u32, kind: Access) -> u8 {
        self.add_wait_cycles(self.wait_time::<1>(addr, kind));
        self.get_byte(addr)
    }

    /// Read a half-word from the bus (LE). Also enforces timing.
    /// Also handles unaligned reads, which is why ret is u32.
    pub(super) fn read_hword(&mut self, addr: u32, kind: Access) -> u32 {
        self.add_wait_cycles(self.wait_time::<2>(addr, kind));
        if addr.is_bit(0) {
            // Unaligned
            let val = self.get_hword(addr - 1);
            Cpu::ror_s0(val.u32(), 8)
        } else {
            // Aligned
            self.get_hword(addr).u32()
        }
    }

    /// Read a half-word from the bus (LE). Also enforces timing.
    /// If address is unaligned, do LDRSH behavior.
    pub(super) fn read_hword_ldrsh(&mut self, addr: u32, kind: Access) -> u32 {
        self.add_wait_cycles(self.wait_time::<2>(addr, kind));
        if addr.is_bit(0) {
            // Unaligned
            let val = self.get_byte(addr);
            val as i8 as i16 as u32
        } else {
            // Aligned
            self.get_hword(addr).u32()
        }
    }

    /// Read a word from the bus (LE). Also enforces timing.
    pub(super) fn read_word(&mut self, addr: u32, kind: Access) -> u32 {
        let addr = addr & !3; // Forcibly align
        self.add_wait_cycles(self.wait_time::<4>(addr, kind));
        self.get_word(addr)
    }

    /// Read a word from the bus (LE). Also enforces timing.
    /// If address is unaligned, do LDR/SWP behavior.
    pub(super) fn read_word_ldrswp(&mut self, addr: u32, kind: Access) -> u32 {
        let val = self.read_word(addr, kind);
        if addr & 3 != 0 {
            // Unaligned
            let by = (addr & 3) << 3;
            Cpu::ror_s0(val, by)
        } else {
            // Aligned
            val
        }
    }

    /// Read a byte from the bus. Does no timing-related things; simply fetches
    /// the value.
    pub(super) fn get_byte(&self, addr: u32) -> u8 {
        let a = addr.us();
        match a {
            0x0400_0000..=0x04FF_FFFF if addr.is_bit(0) => self.get_mmio(addr).high(),
            0x0400_0000..=0x04FF_FFFF => self.get_mmio(addr).low(),

            0x0601_8000..=0x0601_FFFF => self.ppu.vram[0x1_0000 + a & 0x7FFF],
            0x0602_0000..=0x06FF_FFFF => self.get_byte(addr & 0x0601_FFFF),

            _ => unsafe { self.page(a).read() },
        }
    }

    /// Read a half-word from the bus (LE). Does no timing-related things;
    /// simply fetches the value.
    pub(super) fn get_hword(&self, addr: u32) -> u16 {
        let a = addr.us();
        match a {
            0x0400_0000..=0x04FF_FFFF => self.get_mmio(addr),

            0x0601_8000..=0x0601_FFFF => {
                hword(self.get_byte(addr), self.get_byte(addr.wrapping_add(1)))
            }
            0x0602_0000..=0x06FF_FFFF => self.get_hword(addr & 0x0601_FFFF),

            _ => unsafe {
                let ptr = self.page(a);
                let ptr_16 = mem::transmute::<_, *const u16>(ptr);
                ptr_16.read()
            },
        }
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let a = addr & 0x3FE;
        match a {
            // Timers
            TM0CNT_L => self.timers.counters[0],
            TM1CNT_L => self.timers.counters[1],
            TM2CNT_L => self.timers.counters[2],
            TM3CNT_L => self.timers.counters[3],

            _ => self[a],
        }
    }

    /// Read a word from the bus (LE). Does no timing-related things; simply
    /// fetches the value. Also does not handle unaligned reads (yet)
    pub fn get_word(&self, addr: u32) -> u32 {
        let a = addr.us();
        match a {
            0x0400_0000..=0x04FF_FFFF => {
                word(self.get_mmio(addr), self.get_mmio(addr.wrapping_add(2)))
            }

            0x0601_8000..=0x0601_FFFF => {
                word(self.get_hword(addr), self.get_hword(addr.wrapping_add(2)))
            }
            0x0602_0000..=0x06FF_FFFF => self.get_word(addr & 0x0601_FFFF),

            _ => unsafe {
                let ptr = self.page(a);
                let ptr_32 = mem::transmute::<_, *const u32>(ptr);
                ptr_32.read()
            },
        }
    }

    /// Write a byte to the bus. Handles timing.
    pub(super) fn write_byte(&mut self, addr: u32, value: u8, kind: Access) {
        self.add_wait_cycles(self.wait_time::<1>(addr, kind));
        self.set_byte(addr, value)
    }

    /// Write a half-word from the bus (LE). Handles timing.
    pub(super) fn write_hword(&mut self, addr: u32, value: u16, kind: Access) {
        self.add_wait_cycles(self.wait_time::<2>(addr, kind));
        self.set_hword(addr, value)
    }

    /// Write a word from the bus (LE). Handles timing.
    pub(super) fn write_word(&mut self, addr: u32, value: u32, kind: Access) {
        self.add_wait_cycles(self.wait_time::<4>(addr, kind));
        self.set_word(addr, value)
    }

    /// Write a byte to the bus. Does no timing-related things; simply sets the
    /// value.
    pub(super) fn set_byte(&mut self, addr: u32, value: u8) {
        let a = addr.us();
        match a {
            0x0200_0000..=0x02FF_FFFF => self.memory.ewram[a & 0x3FFFF] = value,
            0x0300_0000..=0x03FF_FFFF => self.memory.iwram[a & 0x7FFF] = value,

            0x0400_0000..=0x04FF_FFFF if addr.is_bit(0) => {
                self.set_hword(addr, self.get_hword(addr).set_high(value))
            }
            0x0400_0000..=0x04FF_FFFF => self.set_hword(addr, self.get_hword(addr).set_low(value)),

            // VRAM weirdness
            0x0500_0000..=0x07FF_FFFF => self.set_hword(addr, hword(value, value)),
            _ => (),
        }
    }

    /// Write a half-word from the bus (LE). Does no timing-related things;
    /// simply sets the value.
    pub(super) fn set_hword(&mut self, addr: u32, value: u16) {
        let addr = addr & !1; // Forcibly align: All write instructions do this
        let a = addr.us();
        match a {
            0x0400_0000..=0x04FF_FFFF => self.set_mmio(addr, value),

            0x0500_0000..=0x05FF_FFFF => {
                self.ppu.palette[a & 0x3FF] = value.low();
                self.ppu.palette[(a & 0x3FF) + 1] = value.high();
            }
            0x0600_0000..=0x0601_7FFF => {
                self.ppu.vram[a & 0x17FFF] = value.low();
                self.ppu.vram[(a & 0x17FFF) + 1] = value.high();
            }
            0x0700_0000..=0x07FF_FFFF => {
                self.ppu.oam[a & 0x3FF] = value.low();
                self.ppu.oam[(a & 0x3FF) + 1] = value.high();
            }

            // VRAM mirror weirdness
            0x0601_8000..=0x0601_FFFF => {
                self.ppu.vram[0x1_0000 + a & 0x7FFF] = value.low();
                self.ppu.vram[(0x1_0000 + a & 0x7FFF) + 1] = value.high();
            }
            0x0602_0000..=0x06FF_FFFF => self.set_hword(addr & 0x0601_FFFF, value),

            _ => {
                self.set_byte(addr, value.low());
                self.set_byte(addr.wrapping_add(1), value.high());
            }
        }
    }

    fn set_mmio(&mut self, addr: u32, value: u16) {
        let a = addr & 0x3FF;
        match a {
            // General
            IME => self[IME] = value & 1,
            IF => self[IF] &= !value,

            // PPU
            DISPSTAT => self[DISPSTAT] = (self[DISPSTAT] & 0b111) | (value & !0b11000111),

            // Timers
            TM0CNT_H => Timers::hi_write(self, 0, value),
            TM1CNT_H => Timers::hi_write(self, 1, value),
            TM2CNT_H => Timers::hi_write(self, 2, value),
            TM3CNT_H => Timers::hi_write(self, 3, value),

            // DMAs
            0xBA => {
                self[a] = value;
                Dmas::update_idx(self, 0, value);
            }
            0xC6 => {
                self[a] = value;
                Dmas::update_idx(self, 1, value);
            }
            0xD2 => {
                self[a] = value;
                Dmas::update_idx(self, 2, value);
            }
            0xDE => {
                self[a] = value;
                Dmas::update_idx(self, 3, value);
            }

            // RO registers
            VCOUNT | KEYINPUT => (),

            _ => self[a] = value,
        }
    }

    /// Write a word from the bus (LE). Does no timing-related things; simply
    /// sets the value.
    pub(super) fn set_word(&mut self, addr: u32, value: u32) {
        let addr = addr & !3; // Forcibly align: All write instructions do this
        self.set_hword(addr, value.low());
        self.set_hword(addr.wrapping_add(2), value.high());
    }

    fn page(&self, addr: usize) -> *const u8 {
        let mems = [
            BIOS,
            &self.memory.ewram,
            &self.memory.iwram,
            &[],
            &self.ppu.palette,
            &self.ppu.vram,
            &self.ppu.oam,
            &self.cart.rom,
            &[0, 0, 0, 0], // Need at least 4 due to word reads
        ];
        let page = Region::PAGES[(addr >> 24) & 0xF] as usize;
        let mask = Region::MASK[page];
        unsafe { mems[page].as_ptr().add(addr & mask) }
    }

    const WS_NONSEQ: [u16; 4] = [4, 3, 2, 8];

    fn wait_time<const W: u32>(&self, addr: u32, ty: Access) -> u16 {
        match (addr, W, ty) {
            (0x0200_0000..=0x02FF_FFFF, 4, _) => 6,
            (0x0200_0000..=0x02FF_FFFF, _, _) => 3,
            (0x0500_0000..=0x06FF_FFFF, 4, _) => 2,

            (0x0800_0000..=0x09FF_FFFF, _, Seq) => 2 - self[WAITCNT].bit(4),
            (0x0800_0000..=0x09FF_FFFF, 4, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(2, 2).us()] + (2 - self[WAITCNT].bit(4))
            }
            (0x0800_0000..=0x09FF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(2, 2).us()]
            }

            (0x0A00_0000..=0x0BFF_FFFF, _, Seq) => 4 - (self[WAITCNT].bit(7) * 3),
            (0x0A00_0000..=0x0BFF_FFFF, 4, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(5, 2).us()] + (4 - (self[WAITCNT].bit(7) * 3))
            }
            (0x0A00_0000..=0x0BFF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(5, 2).us()]
            }

            (0x0C00_0000..=0x0DFF_FFFF, _, Seq) => 8 - (self[WAITCNT].bit(10) * 7),
            (0x0C00_0000..=0x0DFF_FFFF, 4, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(8, 2).us()] + (8 - (self[WAITCNT].bit(10) * 7))
            }
            (0x0C00_0000..=0x0DFF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(8, 2).us()]
            }

            _ => 1,
        }
    }
}

impl Index<u32> for GameGirlAdv {
    type Output = u16;

    fn index(&self, addr: u32) -> &Self::Output {
        assert!(addr < 0x3FF);
        assert_eq!(addr & 1, 0);
        &self.memory.mmio[(addr >> 1).us()]
    }
}

impl IndexMut<u32> for GameGirlAdv {
    fn index_mut(&mut self, addr: u32) -> &mut Self::Output {
        assert!(addr < 0x3FF);
        assert_eq!(addr & 1, 0);
        &mut self.memory.mmio[(addr >> 1).us()]
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Region {
    Bios,
    EWRam,
    IWRam,
    IO,
    Palette,
    VRam,
    Oam,
    Rom,
    Unmapped,
}

impl Region {
    const PAGES: [Region; 16] = [
        Region::Bios,
        Region::Unmapped,
        Region::EWRam,
        Region::IWRam,
        Region::IO,
        Region::Palette,
        Region::VRam,
        Region::Oam,
        Region::Rom,
        Region::Rom,
        Region::Rom,
        Region::Rom,
        Region::Rom,
        Region::Rom,
        Region::Unmapped,
        Region::Unmapped,
    ];
    const MASK: [usize; 9] = [
        0x3FFF, 0x3FFFF, 0x7FFF, 0x3FF, 0x3FF, 0x17FFF, 0x3FF, 0x1FF_FFFF, 0,
    ];
}
