use std::{
    fmt::UpperHex,
    mem,
    ops::{Index, IndexMut},
    ptr,
};

use serde::{Deserialize, Serialize};

use super::audio;
use crate::{
    gga::{
        addr::*,
        cpu::Cpu,
        dma::Dmas,
        timer::Timers,
        Access::{self, *},
        GameGirlAdv,
    },
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};

pub const KB: usize = 1024;
pub const PAGE_SIZE: usize = 0x8000; // 32KiB
pub const BIOS: &[u8] = include_bytes!("bios.bin");

/// Memory struct containing the GGA's memory regions along with page tables
/// and other auxiliary cached information relating to memory.
#[derive(Deserialize, Serialize)]
pub struct Memory {
    #[serde(with = "serde_arrays")]
    pub ewram: [u8; 256 * KB],
    #[serde(with = "serde_arrays")]
    pub iwram: [u8; 32 * KB],
    #[serde(with = "serde_arrays")]
    pub mmio: [u16; KB / 2],

    open_bus: [u8; 4],
    #[serde(skip)]
    #[serde(default = "serde_pages")]
    read_pages: [*mut u8; 8192],
    #[serde(skip)]
    #[serde(default = "serde_pages")]
    write_pages: [*mut u8; 8192],

    wait_word: [u16; 32],
    wait_other: [u16; 32],
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
    #[inline]
    pub(super) fn get_byte(&self, addr: u32) -> u8 {
        self.get(addr, |this, addr| match addr {
            0x0400_0000..=0x04FF_FFFF if addr.is_bit(0) => this.get_mmio(addr).high(),
            0x0400_0000..=0x04FF_FFFF => this.get_mmio(addr).low(),
            0x0E00_0000..=0x0E00_FFFF => this.cart.read_ram_byte(addr.us() & 0xFFFF),
            // Account for unmapped last page due to EEPROM
            0x0DFF_8000..=0x0DFF_FFFF if this.cart.rom.len() >= (addr.us() - 0x800_0000) => {
                this.cart.rom[addr.us() - 0x800_0000]
            }
            _ => 0,
        })
    }

    /// Read a half-word from the bus (LE). Does no timing-related things;
    /// simply fetches the value.
    #[inline]
    pub(super) fn get_hword(&self, addr: u32) -> u16 {
        self.get(addr, |this, addr| match addr {
            0x0400_0000..=0x04FF_FFFF => this.get_mmio(addr),
            0x0D00_0000..=0x0DFF_FFFF if this.cart.is_eeprom_at(addr) => this.cart.read_ram_hword(),
            // Account for unmapped last page due to EEPROM
            0x0DFF_8000..=0x0DFF_FFFF => hword(this.get_byte(addr), this.get_byte(addr + 1)),
            _ => 0,
        })
    }

    /// Read a word from the bus (LE). Does no timing-related things; simply
    /// fetches the value. Also does not handle unaligned reads.
    #[inline]
    pub fn get_word(&self, addr: u32) -> u32 {
        self.get(addr, |this, addr| match addr {
            0x0400_0000..=0x04FF_FFFF => {
                word(this.get_mmio(addr), this.get_mmio(addr.wrapping_add(2)))
            }
            // Account for unmapped last page due to EEPROM
            0x0DFF_8000..=0x0DFF_FFFF => word(this.get_hword(addr), this.get_hword(addr + 2)),
            _ => 0,
        })
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let a = addr & 0x3FE;
        match a {
            // Timers
            TM0CNT_L => Timers::time_read::<0>(self),
            TM1CNT_L => Timers::time_read::<1>(self),
            TM2CNT_L => Timers::time_read::<2>(self),
            TM3CNT_L => Timers::time_read::<3>(self),

            // Old sound
            0x60..=0x80 | 0x84 | 0x90..=0x9F => {
                let low = self.apu.cgb_chans.read_register_gga(a.u16());
                let high = self.apu.cgb_chans.read_register_gga(a.u16() + 1);
                hword(low, high)
            }

            _ => self[a],
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
            // DMA channel edge case, why do games do this
            0x0400_00A0..=0x0400_00A3 => self.apu.push_sample::<0>(value),
            0x0400_00A4..=0x0400_00A7 => self.apu.push_sample::<1>(value),

            // HALTCNT
            0x0400_0301 => {
                // We're halted, emulate peripherals until an interrupt is pending
                while self[IF] == 0 {
                    let evt = self.scheduler.pop();
                    evt.kind.dispatch(self, evt.late_by);
                }
            }

            // Old sound
            0x0400_0060..=0x0400_0080 | 0x0400_0084 | 0x0400_0090..=0x0400_009F => {
                self.apu.cgb_chans.write_register_gga(
                    (addr & 0xFFF).u16(),
                    value,
                    &mut audio::shed(&mut self.scheduler),
                )
            }

            // MMIO
            0x0400_0000..=0x04FF_FFFF if addr.is_bit(0) => {
                self.set_hword(addr, self.get_hword(addr).set_high(value))
            }
            0x0400_0000..=0x04FF_FFFF => self.set_hword(addr, self.get_hword(addr).set_low(value)),

            // Cart save
            0x0E00_0000..=0x0E00_FFFF => self.cart.write_ram_byte(addr.us() & 0xFFFF, value),

            // VRAM weirdness
            0x0500_0000..=0x07FF_FFFF => self.set_hword(addr, hword(value, value)),

            _ => self.set(addr, value, |_this, _addr, _value| ()),
        }
    }

    /// Write a half-word from the bus (LE). Does no timing-related things;
    /// simply sets the value.
    pub(super) fn set_hword(&mut self, addr: u32, value: u16) {
        let addr = addr & !1; // Forcibly align: All write instructions do this
        self.set(addr, value, |this, addr, value| match addr {
            0x0400_0000..=0x04FF_FFFF => this.set_mmio(addr, value),
            0x0D00_0000..=0x0DFF_FFFF if this.cart.is_eeprom_at(addr) => {
                this.cart.write_ram_hword(value)
            }
            _ => (),
        });
    }

    /// Write a word from the bus (LE). Does no timing-related things; simply
    /// sets the value.
    pub(super) fn set_word(&mut self, addr: u32, value: u32) {
        let addr = addr & !3; // Forcibly align: All write instructions do this
        self.set(addr, value, |this, addr, value| {
            this.set_hword(addr, value.low());
            this.set_hword(addr.wrapping_add(2), value.high());
        });
    }

    fn set_mmio(&mut self, addr: u32, value: u16) {
        let a = addr & 0x3FF;
        match a {
            // General
            IME => {
                self[IME] = value & 1;
                Cpu::check_if_interrupt(self);
            }
            IE => {
                self[IE] = value;
                Cpu::check_if_interrupt(self);
            }
            IF => self[IF] &= !value,
            WAITCNT => {
                self[a] = value;
                self.update_wait_times();
            }

            // PPU
            DISPSTAT => self[DISPSTAT] = (self[DISPSTAT] & 0b111) | (value & !0b11000111),

            // Timers
            TM0CNT_H => Timers::hi_write::<0>(self, a, value),
            TM1CNT_H => Timers::hi_write::<1>(self, a, value),
            TM2CNT_H => Timers::hi_write::<2>(self, a, value),
            TM3CNT_H => Timers::hi_write::<3>(self, a, value),

            // DMAs
            0xBA => Dmas::update_idx(self, 0, value),
            0xC6 => Dmas::update_idx(self, 1, value),
            0xD2 => Dmas::update_idx(self, 2, value),
            0xDE => Dmas::update_idx(self, 3, value),

            // Audio
            FIFO_A_L | FIFO_A_H => self.apu.push_samples::<0>(value),
            FIFO_B_L | FIFO_B_H => self.apu.push_samples::<1>(value),
            0x60..=0x80 | 0x84 | 0x90..=0x9F => {
                let mut sched = audio::shed(&mut self.scheduler);
                self.apu
                    .cgb_chans
                    .write_register_gga(a.u16(), value.low(), &mut sched);
                self.apu
                    .cgb_chans
                    .write_register_gga(a.u16() + 1, value.high(), &mut sched);
            }

            // RO registers
            VCOUNT | KEYINPUT => (),

            _ => self[a] = value,
        }
    }

    // Unsafe corner!
    /// Get a value in memory. Will try to do a fast read from page tables,
    /// falls back to given closure if no page table is mapped at that address.
    #[inline]
    fn get<T>(&self, addr: u32, slow: fn(&GameGirlAdv, u32) -> T) -> T {
        let ptr = self.page::<false>(addr);
        if ptr as usize > 0x8000 {
            unsafe { mem::transmute::<_, *const T>(ptr).read() }
        } else {
            slow(self, addr)
        }
    }

    /// Sets a value in memory. Will try to do a fast write with page tables,
    /// falls back to given closure if no page table is mapped at that address.
    #[inline]
    fn set<T: UpperHex>(&mut self, addr: u32, value: T, slow: fn(&mut GameGirlAdv, u32, T)) {
        let ptr = self.page::<true>(addr);
        if ptr as usize > 0x8000 {
            unsafe { ptr::write(mem::transmute::<_, *mut T>(ptr), value) }
        } else {
            slow(self, addr, value)
        }
    }

    /// Get the page table at the given address. Can be a write or read table,
    /// see const generic parameter. If there is no page mapped, returns a
    /// pointer in range 0..0x7FFF (due to offsets to the (null) pointer)
    fn page<const WRITE: bool>(&self, addr: u32) -> *mut u8 {
        const MASK: [usize; 16] = [
            0x3FFF, // BIOS
            0,      // Unmapped
            0x7FFF, // EWRAM
            0x7FFF, // IWRAM
            0,      // MMIO
            0x3FF,  // Palette
            0x7FFF, // VRAM
            0x3FF,  // OAM
            0x7FFF, // ROM
            0x7FFF, // ROM
            0x7FFF, // ROM
            0x7FFF, // ROM
            0x7FFF, // ROM
            0x7FFF, // ROM
            0,      // Unmapped
            0,      // Unmapped
        ];
        let addr = addr.us();
        unsafe {
            let mask = MASK.get_unchecked((addr >> 24) & 0xF);
            let page_idx = (addr >> 15) & 8191;
            let page = if WRITE {
                self.memory.write_pages.get_unchecked(page_idx)
            } else {
                self.memory.read_pages.get_unchecked(page_idx)
            };
            page.add(addr & mask)
        }
    }

    /// Get wait time for a given address.
    #[inline]
    pub fn wait_time<const W: u32>(&self, addr: u32, ty: Access) -> u16 {
        let idx = ((addr.us() >> 24) & 0xF) + ty as usize;
        if W == 4 {
            self.memory.wait_word[idx]
        } else {
            self.memory.wait_other[idx]
        }
    }

    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {
        for i in 0..self.memory.read_pages.len() {
            self.memory.read_pages[i] = unsafe { self.get_page::<true>(i * PAGE_SIZE) };
            self.memory.write_pages[i] = unsafe { self.get_page::<false>(i * PAGE_SIZE) };
        }
        self.update_wait_times();
    }

    fn update_wait_times(&mut self) {
        for i in 0..16 {
            let addr = i.u32() * 0x100_0000;
            self.memory.wait_word[i] = self.calc_wait_time::<4>(addr, Seq);
            self.memory.wait_other[i] = self.calc_wait_time::<2>(addr, Seq);
            self.memory.wait_word[i + 16] = self.calc_wait_time::<4>(addr, NonSeq);
            self.memory.wait_other[i + 16] = self.calc_wait_time::<2>(addr, NonSeq);
        }
    }

    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8 {
        unsafe fn offs(reg: &[u8], offs: usize) -> *mut u8 {
            let ptr = reg.as_ptr() as *mut u8;
            ptr.add(offs % reg.len())
        }

        match a {
            0x0000_0000..=0x0000_3FFF if R => offs(BIOS, a),
            0x0200_0000..=0x02FF_FFFF => offs(&self.memory.ewram, a - 0x200_0000),
            0x0300_0000..=0x03FF_FFFF => offs(&self.memory.iwram, a - 0x300_0000),
            0x0500_0000..=0x05FF_FFFF => offs(&self.ppu.palette, a - 0x500_0000),
            0x0600_0000..=0x0601_7FFF => offs(&self.ppu.vram, a - 0x600_0000),
            0x0700_0000..=0x07FF_FFFF => offs(&self.ppu.oam, a - 0x700_0000),
            // Does not go all the way due to EEPROM, also does not mirror
            0x0800_0000..=0x0DFF_7FFF if R && self.cart.rom.len() >= (a - 0x800_0000) => {
                offs(&self.cart.rom, a - 0x800_0000)
            }

            // VRAM mirror weirdness
            0x0601_8000..=0x0601_FFFF => offs(&self.ppu.vram, 0x1_0000 + (a - 0x600_0000)),
            0x0602_0000..=0x06FF_FFFF => self.get_page::<R>(a & 0x601_FFFF),
            _ => ptr::null::<u8>() as *mut u8,
        }
    }

    const WS_NONSEQ: [u16; 4] = [4, 3, 2, 8];

    fn calc_wait_time<const W: u32>(&self, addr: u32, ty: Access) -> u16 {
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

            (0x0E00_0000..=0x0EFF_FFFF, _, _) => Self::WS_NONSEQ[self[WAITCNT].bits(0, 2).us()],

            _ => 1,
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            ewram: [0; 256 * KB],
            iwram: [0; 32 * KB],
            mmio: [0; KB / 2],
            open_bus: [0; 4],
            read_pages: serde_pages(),
            write_pages: serde_pages(),
            wait_word: [0; 32],
            wait_other: [0; 32],
        }
    }
}

unsafe impl Send for Memory {}

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

fn serde_pages() -> [*mut u8; 8192] {
    [ptr::null::<u8>() as *mut u8; 8192]
}
