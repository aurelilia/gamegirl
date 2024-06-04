// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    ops::{Index, IndexMut},
    ptr,
};

use arm_cpu::{
    registers::Flag,
    Access::{self, *},
    Cpu, Interrupt,
};
use common::{
    components::memory::{MemoryMappedSystem, MemoryMapper},
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};

use super::audio;
use crate::{addr::*, dma::Dmas, timer::Timers, Apu, GameGirlAdv};

pub const KB: usize = 1024;
pub const BIOS: &[u8] = include_bytes!("bios.bin");

/// Memory struct containing the GGA's memory regions along with page tables
/// and other auxiliary cached information relating to memory.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub ewram: [u8; 256 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub iwram: [u8; 32 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub mmio: [u16; KB / 2],

    /// Value to return when trying to read BIOS outside of it
    pub(crate) bios_value: u32,
    /// Length of the prefetch buffer at the current PC.
    pub(crate) prefetch_len: u16,

    mapper: MemoryMapper<8192>,
    wait_word: [u16; 32],
    wait_other: [u16; 32],
}

impl GameGirlAdv {
    /// Read a byte from the bus. Does no timing-related things; simply fetches
    /// the value.
    #[inline]
    pub fn get_byte(&self, addr: u32) -> u8 {
        MemoryMapper::get(self, addr, !0, |this, addr| match addr {
            0x0000_0000..=0x0000_3FFF if this.cpu.pc() < 0x0100_0000 => Self::bios_read(addr),
            0x0000_0000..=0x0000_3FFF => this.memory.bios_value.u8(),

            0x0400_0000..=0x04FF_FFFF if addr.is_bit(0) => this.get_mmio(addr).high(),
            0x0400_0000..=0x04FF_FFFF => this.get_mmio(addr).low(),

            0x0E00_0000..=0x0FFF_FFFF => this.cart.read_ram_byte(addr.us() & 0xFFFF),
            // Account for unmapped last page due to EEPROM
            0x0DFF_8000..=0x0DFF_FFFF if this.cart.rom.len() >= (addr.us() - 0x800_0000) => {
                this.cart.rom[addr.us() - 0x800_0000]
            }

            _ => this.invalid_read::<false>(addr).u8(),
        })
    }

    /// Read a half-word from the bus (LE). Does no timing-related things;
    /// simply fetches the value.
    #[inline]
    pub(super) fn get_hword(&self, addr: u32) -> u16 {
        MemoryMapper::get(self, addr, !1, |this, addr| match addr {
            0x0000_0000..=0x0000_3FFF if this.cpu.pc() < 0x0100_0000 => Self::bios_read(addr),
            0x0000_0000..=0x0000_3FFF => this.memory.bios_value.u16(),

            0x0400_0000..=0x04FF_FFFF => this.get_mmio(addr),

            // If EEPROM, use that...
            0x0D00_0000..=0x0DFF_FFFF if this.cart.is_eeprom_at(addr) => this.cart.read_ram_hword(),
            // If not, account for unmapped last page due to EEPROM
            0x0DFF_8000..=0x0DFF_FFFF => hword(this.get_byte(addr), this.get_byte(addr + 1)),

            // Other saves
            0x0E00_0000..=0x0FFF_FFFF => {
                // Reading halfwords causes the byte to be repeated
                let byte = this.cart.read_ram_byte(addr.us() & 0xFFFF);
                hword(byte, byte)
            }

            _ => this.invalid_read::<false>(addr).u16(),
        })
    }

    /// Read a word from the bus (LE). Does no timing-related things; simply
    /// fetches the value. Also does not handle unaligned reads.
    #[inline]
    pub fn get_word(&self, addr: u32) -> u32 {
        MemoryMapper::get(self, addr, !3, |this, addr| match addr {
            0x0000_0000..=0x0000_3FFF if this.cpu.pc() < 0x0100_0000 => Self::bios_read(addr),
            0x0000_0000..=0x0000_3FFF => this.memory.bios_value,

            0x0400_0000..=0x04FF_FFFF => {
                word(this.get_mmio(addr), this.get_mmio(addr.wrapping_add(2)))
            }

            // Account for unmapped last page due to EEPROM
            0x0DFF_8000..=0x0DFF_FFFF => word(this.get_hword(addr), this.get_hword(addr + 2)),

            // Other saves
            0x0E00_0000..=0x0FFF_FFFF => {
                // Reading words causes the byte to be repeated
                let byte = this.cart.read_ram_byte(addr.us() & 0xFFFF);
                let hword = hword(byte, byte);
                word(hword, hword)
            }

            _ => this.invalid_read::<true>(addr),
        })
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        if addr == 0x400_100C {
            // annoying edge case register
            return self.invalid_read::<false>(addr).u16();
        }

        let a = addr & 0x3FE;
        match a {
            // Timers
            TM0CNT_L => Timers::time_read::<0>(self),
            TM1CNT_L => Timers::time_read::<1>(self),
            TM2CNT_L => Timers::time_read::<2>(self),
            TM3CNT_L => Timers::time_read::<3>(self),

            // PPU
            DISPCNT..BLDALPHA if let Some(val) = self.ppu.read_mmio(a) => val,

            // Old sound
            0x60..=0x80 | 0x84 | 0x86 | 0x8A | 0x90..=0x9F => {
                let low = Apu::read_register_psg(&self.apu.cgb_chans, a.u16());
                let high = Apu::read_register_psg(&self.apu.cgb_chans, a.u16() + 1);
                hword(low, high)
            }
            // Sound register with some write-only bits
            SOUNDCNT_H => self[a] & 0x770F,
            // Zero registers (DMA)
            0xB8 | 0xC4 | 0xD0 | 0xDC => 0,

            BG0HOFS..=WIN1V
            | MOSAIC
            | BLDY
            | 0xB0..=0xB7
            | 0xBC..=0xC3
            | 0xC8..=0xCF
            | 0xD4..=0xDB
            | 0x4E
            | 0x56..=0x5E
            | 0x8C..=0x8E
            | 0xA0..=0xAF
            | 0xE0..=0xFF
            | 0x110..=0x12F
            | 0x134
            | 0x138..=0x140
            | 0x144..=0x158
            | 0x15C..=0x1FF
            | 0x304..=0x3FE => self.invalid_read::<false>(addr).u16(),

            _ => self[a],
        }
    }

    fn invalid_read<const WORD: bool>(&self, addr: u32) -> u32 {
        match addr {
            0x0800_0000..=0x0DFF_FFFF => {
                // Out of bounds ROM read
                let addr = (addr & !if WORD { 3 } else { 1 }) >> 1;
                let low = addr.u16();
                word(low, low.wrapping_add(1))
            }

            _ if self.cpu.pc() == self.dma.pc_at_last_end => self.dma.cache,

            _ => {
                // Open bus
                if self.cpu.pc() > 0xFFF_FFFF
                    || (self.cpu.pc() > 0x3FFF && self.cpu.pc() < 0x200_0000)
                {
                    return 0;
                }

                if !self.cpu.flag(Flag::Thumb) {
                    // Simple case: just read PC in ARM mode
                    self.get_word(self.cpu.pc())
                } else {
                    // Thumb mode... complicated.
                    // https://problemkaputt.de/gbatek.htm#gbaunpredictablethings
                    match self.cpu.pc() >> 24 {
                        0x02 | 0x05 | 0x06 | 0x08..=0xD => {
                            let hword = self.get_hword(self.cpu.pc());
                            word(hword, hword)
                        }
                        _ if self.cpu.pc().is_bit(1) => word(
                            self.get_hword(self.cpu.pc() - 2),
                            self.get_hword(self.cpu.pc()),
                        ),
                        0x00 | 0x07 => word(
                            self.get_hword(self.cpu.pc()),
                            self.get_hword(self.cpu.pc() + 2),
                        ),
                        _ => word(
                            self.get_hword(self.cpu.pc()),
                            self.get_hword(self.cpu.pc() - 2),
                        ),
                    }
                }
            }
        }
    }

    /// Write a byte to the bus. Does no timing-related things; simply sets the
    /// value.
    pub fn set_byte(&mut self, addr: u32, value: u8) {
        let a = addr.us();
        match a {
            // DMA channel edge case, why do games do this
            0x0400_00A0..=0x0400_00A3 => self.apu.push_sample::<0>(value),
            0x0400_00A4..=0x0400_00A7 => self.apu.push_sample::<1>(value),

            // HALTCNT
            0x0400_0301 => {
                self.cpu.is_halted = true;
            }

            // Old sound
            0x0400_0060..=0x0400_0080 | 0x0400_0084 | 0x0400_0090..=0x0400_009F => {
                Apu::write_register_psg(
                    &mut self.apu.cgb_chans,
                    (addr & 0xFFF).u16(),
                    value,
                    &mut audio::shed(&mut self.scheduler),
                );
            }

            // MMIO
            0x0400_0000..=0x0400_0301 if addr.is_bit(0) => {
                self.set_hword(addr, self.get_hword(addr).set_high(value));
            }
            0x0400_0000..=0x0400_0301 => self.set_hword(addr, self.get_hword(addr).set_low(value)),

            // Cart save
            0x0E00_0000..=0x0FFF_FFFF => self.cart.write_ram_byte(addr.us() & 0xFFFF, value),

            // VRAM weirdness
            0x0500_0000..=0x0600_FFFF => self.set_hword(addr & !1, hword(value, value)),
            0x0602_0000..=0x06FF_FFFF if a & 0x1_FFFF < 0x1_0000 => {
                // Only BG VRAM gets written to, OBJ VRAM is ignored
                self.set_hword(addr & !1, hword(value, value));
            }
            0x0601_0000..=0x07FF_FFFF => (), // Ignored

            _ => MemoryMapper::set(self, addr, value, |_this, _addr, _value| ()),
        }
        self.cpu.cache.write(addr);
    }

    /// Write a half-word from the bus (LE). Does no timing-related things;
    /// simply sets the value.
    pub(super) fn set_hword(&mut self, addr_unaligned: u32, value: u16) {
        let addr = addr_unaligned & !1; // Forcibly align: All write instructions do this
        MemoryMapper::set(self, addr, value, |this, addr, value| match addr {
            0x0400_0000..=0x0400_0300 => this.set_mmio(addr, value),

            // Maybe write EEPROM
            0x0D00_0000..=0x0DFF_FFFF if this.cart.is_eeprom_at(addr) => {
                this.cart.write_ram_hword(value);
            }

            // Other saves
            0x0E00_0000..=0x0FFF_FFFF => {
                // Writing halfwords causes a byte from it to be written
                let byte = if addr_unaligned.is_bit(0) {
                    value.high()
                } else {
                    value.low()
                };
                this.cart.write_ram_byte(addr_unaligned.us() & 0xFFFF, byte);
            }

            _ => (),
        });
        self.cpu.cache.write(addr);
    }

    /// Write a word from the bus (LE). Does no timing-related things; simply
    /// sets the value.
    pub(super) fn set_word(&mut self, addr_unaligned: u32, value: u32) {
        let addr = addr_unaligned & !3; // Forcibly align: All write instructions do this
        MemoryMapper::set(self, addr, value, |this, addr, value| match addr {
            0x0400_0000..=0x0400_0300 => {
                this.set_mmio(addr, value.low());
                this.set_mmio(addr.wrapping_add(2), value.high());
            }

            // Saves
            0x0E00_0000..=0x0FFF_FFFF => {
                // Writing words causes a byte from it to be written
                let byte_shift = (addr_unaligned & 3) * 8;
                let byte = (value >> byte_shift) & 0xFF;
                this.cart
                    .write_ram_byte(addr_unaligned.us() & 0xFFFF, byte.u8());
            }

            _ => (),
        });
        self.cpu.cache.write(addr);
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
            IF => {
                self[IF] &= !value;
                // We assume that acknowledging the interrupt is the last thing the handler
                // does, and set the BIOS read value to the post-interrupt
                // state. Not entirely accurate...
                if self.memory.bios_value == 0xE25E_F004 {
                    self.memory.bios_value = 0xE55E_C002;
                }
            }
            WAITCNT => {
                let prev = self[a];
                self[a] = value;
                // Only update things as needed
                if value.bits(0, 11) != prev.bits(0, 11) {
                    self.update_wait_times();
                }
                if value.bit(14) != prev.bit(14) {
                    self.memory.prefetch_len = 0;
                    self.cpu.cache.invalidate_rom();
                } else if value.bits(2, 9) != prev.bits(2, 9) {
                    self.cpu.cache.invalidate_rom();
                }
            }

            // DMA Audio
            FIFO_A_L | FIFO_A_H => self.apu.push_samples::<0>(value),
            FIFO_B_L | FIFO_B_H => self.apu.push_samples::<1>(value),

            // PPU
            DISPCNT..=BLDY => self.ppu.write_mmio(a, value),

            // Timers
            TM0CNT_H => Timers::hi_write::<0>(self, a, value),
            TM1CNT_H => Timers::hi_write::<1>(self, a, value),
            TM2CNT_H => Timers::hi_write::<2>(self, a, value),
            TM3CNT_H => Timers::hi_write::<3>(self, a, value),

            // DMAs
            0xBA => Dmas::ctrl_write(self, 0, value),
            0xC6 => Dmas::ctrl_write(self, 1, value),
            0xD2 => Dmas::ctrl_write(self, 2, value),
            0xDE => Dmas::ctrl_write(self, 3, value),

            // Joypad control
            KEYCNT => {
                self[a] = value;
                self.check_keycnt();
            }

            // CGB audio
            0x60..=0x80 | 0x84 | 0x90..=0x9F => {
                let mut sched = audio::shed(&mut self.scheduler);
                Apu::write_register_psg(&mut self.apu.cgb_chans, a.u16(), value.low(), &mut sched);
                Apu::write_register_psg(
                    &mut self.apu.cgb_chans,
                    a.u16() + 1,
                    value.high(),
                    &mut sched,
                );
            }

            // RO registers
            KEYINPUT | 0x136 | 0x142 | 0x15A | 0x206 | 0x20A | 0x302 => (),

            // Serial
            // TODO this is not how serial actually works but it tricks some tests...
            SIOCNT => {
                self[a] = value.set_bit(7, false);
                if value == 0x4003 {
                    Cpu::request_interrupt(self, Interrupt::Serial);
                }
            }

            _ => self[a] = value,
        }
    }

    fn bios_read<T>(addr: u32) -> T {
        unsafe {
            let ptr = BIOS.as_ptr().add(addr.us() & 0x3FFF);
            ptr.cast::<T>().read()
        }
    }

    /// Get wait time for a given address.
    #[inline]
    pub fn wait_time<T: NumExt + 'static>(&mut self, addr: u32, ty: Access) -> u16 {
        let prefetch_size = if T::WIDTH == 4 { 2 } else { 1 };
        if addr == self.cpu.pc() && self.memory.prefetch_len >= prefetch_size {
            self.memory.prefetch_len -= prefetch_size;
            return prefetch_size;
        }

        let idx = ((addr.us() >> 24) & 0xF) + ty as usize;
        if T::WIDTH == 4 {
            self.memory.wait_word[idx]
        } else {
            self.memory.wait_other[idx]
        }
    }

    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {
        MemoryMapper::init_pages(self);
        self.update_wait_times();
        if self.config.cached_interpreter {
            self.cpu.cache.init(self.cart.rom.len());
        }
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

    const WS_NONSEQ: [u16; 4] = [5, 4, 3, 9];

    fn calc_wait_time<const W: u32>(&self, addr: u32, ty: Access) -> u16 {
        match (addr, W, ty) {
            (0x0200_0000..=0x02FF_FFFF, 4, _) => 6,
            (0x0200_0000..=0x02FF_FFFF, _, _) => 3,
            (0x0500_0000..=0x06FF_FFFF, 4, _) => 2,

            (0x0800_0000..=0x0DFF_FFFF, 4, _) => {
                // Cart bus is 16bit, word access is therefore 2x
                self.calc_wait_time::<2>(addr, ty) + self.calc_wait_time::<2>(addr, Seq)
            }

            (0x0800_0000..=0x09FF_FFFF, _, Seq) => 3 - self[WAITCNT].bit(4),
            (0x0800_0000..=0x09FF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(2, 2).us()]
            }

            (0x0A00_0000..=0x0BFF_FFFF, _, Seq) => 5 - (self[WAITCNT].bit(7) * 3),
            (0x0A00_0000..=0x0BFF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self[WAITCNT].bits(5, 2).us()]
            }

            (0x0C00_0000..=0x0DFF_FFFF, _, Seq) => 9 - (self[WAITCNT].bit(10) * 7),
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
            bios_value: 0xE129_F000,
            prefetch_len: 0,
            mapper: MemoryMapper::default(),
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

impl MemoryMappedSystem<8192> for GameGirlAdv {
    type Usize = u32;
    const ADDR_MASK: &'static [usize] = &[
        0,      // Unmapped
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
    const PAGE_POW: usize = 15;
    const MASK_POW: usize = 24;

    fn get_mapper(&self) -> &MemoryMapper<8192> {
        &self.memory.mapper
    }

    fn get_mapper_mut(&mut self) -> &mut MemoryMapper<8192> {
        &mut self.memory.mapper
    }

    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8 {
        unsafe fn offs(reg: &[u8], offs: usize) -> *mut u8 {
            let ptr = reg.as_ptr() as *mut u8;
            ptr.add(offs % reg.len())
        }

        match a {
            0x0200_0000..=0x02FF_FFFF => offs(&self.memory.ewram, a - 0x200_0000),
            0x0300_0000..=0x03FF_FFFF => offs(&self.memory.iwram, a - 0x300_0000),
            0x0500_0000..=0x05FF_FFFF => offs(&self.ppu.palette, a - 0x500_0000),
            0x0600_0000..=0x0601_7FFF => offs(&self.ppu.vram, a - 0x600_0000),
            0x0700_0000..=0x07FF_FFFF => offs(&self.ppu.oam, a - 0x700_0000),
            0x0800_0000..=0x09FF_FFFF if R && self.cart.rom.len() >= (a - 0x800_0000) => {
                offs(&self.cart.rom, a - 0x800_0000)
            }
            0x0A00_0000..=0x0BFF_FFFF if R && self.cart.rom.len() >= (a - 0xA00_0000) => {
                offs(&self.cart.rom, a - 0xA00_0000)
            }
            // Does not go all the way due to EEPROM
            0x0C00_0000..=0x0DFF_7FFF if R && self.cart.rom.len() >= (a - 0xC00_0000) => {
                offs(&self.cart.rom, a - 0xC00_0000)
            }

            // VRAM mirror weirdness
            0x0601_8000..=0x0601_FFFF => offs(&self.ppu.vram, 0x1_0000 + (a - 0x600_0000)),
            0x0602_0000..=0x06FF_FFFF => self.get_page::<R>(a & 0x601_FFFF),
            _ => ptr::null::<u8>() as *mut u8,
        }
    }
}
