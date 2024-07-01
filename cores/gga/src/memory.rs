// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, self file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with self file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::ptr;

use arm_cpu::{
    registers::Flag,
    Access::{self, *},
    Cpu, Interrupt,
};
use common::{
    components::{
        debugger::Severity,
        memory::{MemoryMappedSystem, MemoryMapper},
    },
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};

use super::audio;
use crate::{addr::*, dma::Dmas, input::KeyControl, timer::Timers, Apu, GameGirlAdv};

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

    // Various registers
    pub keycnt: KeyControl,
    pub waitcnt: u16,

    /// Value to return when trying to read BIOS outside of it
    pub(crate) bios_value: u32,
    /// Length of the prefetch buffer at the current PC.
    pub(crate) prefetch_len: u16,

    pub mapper: MemoryMapper<8192>,
    wait_word: [u16; 32],
    wait_other: [u16; 32],
}

impl GameGirlAdv {
    /// Read a byte from the bus. Does no timing-related things; simply fetches
    /// the value.
    #[inline]
    pub fn get_byte(&self, addr: u32) -> u8 {
        self.memory.mapper.get::<Self, _>(addr).unwrap_or_else(|| {
            match addr {
                0x0000_0000..=0x0000_3FFF if self.cpu.pc() < 0x0100_0000 => Self::bios_read(addr),
                0x0000_0000..=0x0000_3FFF => self.memory.bios_value.u8(),

                0x0400_0000..=0x04FF_FFFF if addr.is_bit(0) => self.get_mmio(addr).high(),
                0x0400_0000..=0x04FF_FFFF => self.get_mmio(addr).low(),

                0x0E00_0000..=0x0FFF_FFFF => self.cart.read_ram_byte(addr.us() & 0xFFFF),
                // Account for unmapped last page due to EEPROM
                0x0DFF_8000..=0x0DFF_FFFF if self.cart.rom.len() >= (addr.us() - 0x800_0000) => {
                    self.cart.rom[addr.us() - 0x800_0000]
                }

                _ => self.invalid_read::<false>(addr).u8(),
            }
        })
    }

    /// Read a half-word from the bus (LE). Does no timing-related things;
    /// simply fetches the value.
    #[inline]
    pub(super) fn get_hword(&self, addr: u32) -> u16 {
        let addr = addr & !1;
        self.memory.mapper.get::<Self, _>(addr).unwrap_or_else(|| {
            match addr {
                0x0000_0000..=0x0000_3FFF if self.cpu.pc() < 0x0100_0000 => Self::bios_read(addr),
                0x0000_0000..=0x0000_3FFF => self.memory.bios_value.u16(),

                0x0400_0000..=0x04FF_FFFF => self.get_mmio(addr),

                // If EEPROM, use that...
                0x0D00_0000..=0x0DFF_FFFF if self.cart.is_eeprom_at(addr) => {
                    self.cart.read_ram_hword()
                }
                // If not, account for unmapped last page due to EEPROM
                0x0DFF_8000..=0x0DFF_FFFF => hword(self.get_byte(addr), self.get_byte(addr + 1)),

                // Other saves
                0x0E00_0000..=0x0FFF_FFFF => {
                    // Reading halfwords causes the byte to be repeated
                    let byte = self.cart.read_ram_byte(addr.us() & 0xFFFF);
                    hword(byte, byte)
                }

                _ => self.invalid_read::<false>(addr).u16(),
            }
        })
    }

    /// Read a word from the bus (LE). Does no timing-related things; simply
    /// fetches the value. Also does not handle unaligned reads.
    #[inline]
    pub fn get_word(&self, addr: u32) -> u32 {
        let addr = addr & !3;
        self.memory.mapper.get::<Self, _>(addr).unwrap_or_else(|| {
            match addr {
                0x0000_0000..=0x0000_3FFF if self.cpu.pc() < 0x0100_0000 => Self::bios_read(addr),
                0x0000_0000..=0x0000_3FFF => self.memory.bios_value,

                0x0400_0000..=0x04FF_FFFF => {
                    word(self.get_mmio(addr), self.get_mmio(addr.wrapping_add(2)))
                }

                // Account for unmapped last page due to EEPROM
                0x0DFF_8000..=0x0DFF_FFFF => word(self.get_hword(addr), self.get_hword(addr + 2)),

                // Other saves
                0x0E00_0000..=0x0FFF_FFFF => {
                    // Reading words causes the byte to be repeated
                    let byte = self.cart.read_ram_byte(addr.us() & 0xFFFF);
                    let hword = hword(byte, byte);
                    word(hword, hword)
                }

                _ => self.invalid_read::<true>(addr),
            }
        })
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let a = addr & 0x1FFE;
        match a {
            // Timers
            TM0CNT_L => Timers::time_read::<0>(self),
            TM1CNT_L => Timers::time_read::<1>(self),
            TM2CNT_L => Timers::time_read::<2>(self),
            TM3CNT_L => Timers::time_read::<3>(self),
            TM0CNT_H => self.timers.control[0].into(),
            TM1CNT_H => self.timers.control[1].into(),
            TM2CNT_H => self.timers.control[2].into(),
            TM3CNT_H => self.timers.control[3].into(),

            // PPU
            DISPCNT..=BLDALPHA if let Some(val) = self.ppu.regs.read_mmio(a) => val,

            // Sound
            0x60..=0x80 | 0x84 | 0x86 | 0x8A | 0x90..=0x9F => {
                let low = Apu::read_register_psg(&self.apu.cgb_chans, a.u16());
                let high = Apu::read_register_psg(&self.apu.cgb_chans, a.u16() + 1);
                hword(low, high)
            }
            SOUNDCNT_H => Into::<u16>::into(self.apu.cnt) & 0x770F,
            SOUNDBIAS_L => self.apu.bias.into(),

            // Keyinput
            KEYINPUT => self.keyinput(),

            // Interrupt control
            IME => self.cpu.ime as u16,
            IE => self.cpu.ie.low(),
            IF => self.cpu.if_.low(),
            WAITCNT => self.memory.waitcnt,

            // DMA
            0xBA => Into::<u16>::into(self.dma.channels[0].ctrl) & 0xF7E0,
            0xC6 => Into::<u16>::into(self.dma.channels[1].ctrl) & 0xF7E0,
            0xD2 => Into::<u16>::into(self.dma.channels[2].ctrl) & 0xF7E0,
            0xDE => Into::<u16>::into(self.dma.channels[3].ctrl) & 0xFFE0,
            // DMA length registers read 0
            0xB8 | 0xC4 | 0xD0 | 0xDC => 0,

            // Known 0 registers
            0x136 | 0x142 | 0x15A | 0x206 | 0x20A | POSTFLG | 0x302 => 0,

            // Known invalid read registers
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
            | 0x304..=0x3FE
            | 0x100C => {
                self.debugger.log(
                    "invalid-mmio-read-known",
                    format!(
                        "Read from write-only/shadow IO register 0x{a:03X}, returning open bus"
                    ),
                    Severity::Info,
                );
                self.invalid_read::<false>(addr).u16()
            }

            _ => {
                self.debugger.log(
                    "invalid-mmio-read-unknown",
                    format!("Read from unknown IO register 0x{a:03X}, returning open bus"),
                    Severity::Warning,
                );
                self.invalid_read::<false>(addr).u16()
            }
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
            0x0500_0000..=0x0600_FFFF if !self.ppu.regs.is_bitmap_mode() => {
                self.set_hword(addr & !1, hword(value, value))
            }
            0x0500_0000..=0x0601_3FFF => self.set_hword(addr & !1, hword(value, value)),
            0x0602_0000..=0x06FF_FFFF if a & 0x1_FFFF < 0x1_0000 => {
                // Only BG VRAM gets written to, OBJ VRAM is ignored
                self.set_hword(addr & !1, hword(value, value));
            }
            0x0601_0000..=0x07FF_FFFF if !self.ppu.regs.is_bitmap_mode() => (), // Ignored
            0x0601_4000..=0x07FF_FFFF => (),                                    // Ignored

            _ => {
                self.memory.mapper.set::<Self, _>(addr, value);
            }
        }
        self.cpu.cache.write(addr);
    }

    /// Write a half-word from the bus (LE). Does no timing-related things;
    /// simply sets the value.
    pub(super) fn set_hword(&mut self, addr_unaligned: u32, value: u16) {
        let addr = addr_unaligned & !1; // Forcibly align: All write instructions do this
        let success = self.memory.mapper.set::<Self, _>(addr, value);
        if !success {
            match addr {
                0x0400_0000..=0x0400_0300 => self.set_mmio(addr, value),

                // Maybe write EEPROM
                0x0D00_0000..=0x0DFF_FFFF if self.cart.is_eeprom_at(addr) => {
                    self.cart.write_ram_hword(value);
                }

                // Other saves
                0x0E00_0000..=0x0FFF_FFFF => {
                    // Writing halfwords causes a byte from it to be written
                    let byte = if addr_unaligned.is_bit(0) {
                        value.high()
                    } else {
                        value.low()
                    };
                    self.cart.write_ram_byte(addr_unaligned.us() & 0xFFFF, byte);
                }

                _ => (),
            }
        }
        self.cpu.cache.write(addr);
    }

    /// Write a word from the bus (LE). Does no timing-related things; simply
    /// sets the value.
    pub(super) fn set_word(&mut self, addr_unaligned: u32, value: u32) {
        let addr = addr_unaligned & !3; // Forcibly align: All write instructions do this
        let success = self.memory.mapper.set::<Self, _>(addr, value);
        if !success {
            match addr {
                0x0400_0000..=0x0400_0300 => {
                    self.set_mmio(addr, value.low());
                    self.set_mmio(addr.wrapping_add(2), value.high());
                }

                // Saves
                0x0E00_0000..=0x0FFF_FFFF => {
                    // Writing words causes a byte from it to be written
                    let byte_shift = (addr_unaligned & 3) * 8;
                    let byte = (value >> byte_shift) & 0xFF;
                    self.cart
                        .write_ram_byte(addr_unaligned.us() & 0xFFFF, byte.u8());
                }

                _ => (),
            };
        }
        self.cpu.cache.write(addr);
    }

    fn set_mmio(&mut self, addr: u32, value: u16) {
        let a = addr & 0x3FF;
        match a {
            // General
            IME => {
                self.cpu.ime = value.is_bit(0);
                Cpu::check_if_interrupt(self);
            }
            IE => {
                self.cpu.ie = value.u32();
                Cpu::check_if_interrupt(self);
            }
            IF => {
                self.cpu.if_ &= !(value.u32());
                // We assume that acknowledging the interrupt is the last thing the handler
                // does, and set the BIOS read value to the post-interrupt
                // state. Not entirely accurate...
                if self.memory.bios_value == 0xE25E_F004 {
                    self.memory.bios_value = 0xE55E_C002;
                }
            }
            WAITCNT => {
                let prev = self.memory.waitcnt;
                self.memory.waitcnt = value;
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
            SOUNDCNT_H => self.apu.cnt = value.into(),
            SOUNDBIAS_L => self.apu.bias = value.into(),

            // PPU
            DISPCNT..=BLDY => self.ppu.regs.write_mmio(a, value),

            // Timers
            TM0CNT_L => self.timers.reload[0] = value,
            TM1CNT_L => self.timers.reload[1] = value,
            TM2CNT_L => self.timers.reload[2] = value,
            TM3CNT_L => self.timers.reload[3] = value,
            TM0CNT_H => Timers::hi_write::<0>(self, value),
            TM1CNT_H => Timers::hi_write::<1>(self, value),
            TM2CNT_H => Timers::hi_write::<2>(self, value),
            TM3CNT_H => Timers::hi_write::<3>(self, value),

            // DMAs
            0xB0..0xE0 => {
                let idx = (a.us() - 0xB0) / 12;
                let reg = (a - 0xB0) % 12;

                let dma = &mut self.dma.channels[idx];
                match reg {
                    0x0 => dma.sad = dma.sad.set_low(value),
                    0x2 => dma.sad = dma.sad.set_high(value),
                    0x4 => dma.dad = dma.dad.set_low(value),
                    0x6 => dma.dad = dma.dad.set_high(value),
                    0x8 => dma.count = value,
                    0xA => Dmas::ctrl_write(self, idx, value),
                    _ => unreachable!(),
                }
            }

            // Joypad control
            KEYCNT => {
                self.memory.keycnt = value.into();
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

            // Serial
            // TODO this is not how serial actually works but it tricks some tests...
            SIOCNT => {
                if value == 0x4003 {
                    Cpu::request_interrupt(self, Interrupt::Serial);
                }
            }

            // RO registers, or otherwise invalid
            KEYINPUT
            | 0x86
            | 0x134
            | 0x136
            | 0x15A
            | 0x206
            | 0x300
            | 0x302
            | 0x56..=0x5E
            | 0x8A..=0x8E
            | 0xA8..=0xAF
            | 0xE0..=0xFF
            | 0x110..=0x12E
            | 0x140..=0x15E
            | 0x20A..=0x21E => self.debugger.log(
                "invalid-mmio-write-known",
                format!(
                    "Write to known read-only IO register 0x{a:03X} (value {value:04X}), ignoring"
                ),
                Severity::Info,
            ),

            _ => self.debugger.log(
                "invalid-mmio-write-unknown",
                format!("Write to unknown IO register 0x{a:03X} (value {value:04X}), ignoring"),
                Severity::Warning,
            ),
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

            (0x0800_0000..=0x09FF_FFFF, _, Seq) => 3 - self.memory.waitcnt.bit(4),
            (0x0800_0000..=0x09FF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self.memory.waitcnt.bits(2, 2).us()]
            }

            (0x0A00_0000..=0x0BFF_FFFF, _, Seq) => 5 - (self.memory.waitcnt.bit(7) * 3),
            (0x0A00_0000..=0x0BFF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self.memory.waitcnt.bits(5, 2).us()]
            }

            (0x0C00_0000..=0x0DFF_FFFF, _, Seq) => 9 - (self.memory.waitcnt.bit(10) * 7),
            (0x0C00_0000..=0x0DFF_FFFF, _, NonSeq) => {
                Self::WS_NONSEQ[self.memory.waitcnt.bits(8, 2).us()]
            }

            (0x0E00_0000..=0x0EFF_FFFF, _, _) => {
                Self::WS_NONSEQ[self.memory.waitcnt.bits(0, 2).us()]
            }

            _ => 1,
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            ewram: [0; 256 * KB],
            iwram: [0; 32 * KB],
            keycnt: 0.into(),
            waitcnt: 0,
            bios_value: 0xE129_F000,
            prefetch_len: 0,
            mapper: MemoryMapper::default(),
            wait_word: [0; 32],
            wait_other: [0; 32],
        }
    }
}

unsafe impl Send for Memory {}

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
