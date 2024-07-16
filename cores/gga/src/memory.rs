// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, self file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with self file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::ptr;

use arm_cpu::{
    access::{CODE, NONSEQ, SEQ},
    interface::{ArmSystem, RwType},
    registers::Flag,
    Access, Cpu, Interrupt,
};
use common::{
    common::debugger::Severity,
    components::memory_mapper::{MemoryMappedSystem, MemoryMapper},
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};
use modular_bitfield::{bitfield, specifiers::*};

use super::audio;
use crate::{addr::*, bios::BIOS, dma::Dmas, input::KeyControl, Apu, GameGirlAdv};

pub const KB: usize = 1024;

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct WaitCnt {
    sram: B2,
    ws0_n: B2,
    ws0_s: B1,
    ws1_n: B2,
    ws1_s: B1,
    ws2_n: B2,
    ws2_s: B1,
    #[skip]
    phi: B2,
    #[skip]
    __: B1,
    prefetch_en: bool,
    #[skip]
    __: B1,
}

#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Prefetch {
    active: bool,
    restart: bool,
    thumb: bool,

    head: u32,
    tail: u32,

    count: u32,
    countdown: i16,
    duty: u16,
}

/// Memory struct containing the GGA's memory regions along with page tables
/// and other auxiliary cached information relating to memory.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    pub bios: Box<[u8]>,
    pub ewram: Box<[u8]>,
    pub iwram: Box<[u8]>,

    // Various registers
    pub keycnt: KeyControl,
    pub keys_prev: u16,
    pub waitcnt: WaitCnt,
    /// Value to return when trying to read BIOS outside of it
    pub(crate) bios_value: u32,

    pub mapper: MemoryMapper<8192>,
    pub(crate) prefetch: Prefetch,
    wait_word: [u16; 32],
    wait_other: [u16; 32],
}

impl GameGirlAdv {
    pub fn get<T: RwType>(&self, addr_unaligned: u32) -> T {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        if addr >= 0x1000_0000 {
            return T::from_u32(self.invalid_read::<false>(addr));
        }

        self.memory.mapper.get::<Self, _>(addr).unwrap_or_else(|| {
            match addr {
                // BIOS
                0x0000_0000..=0x0000_3FFF if self.cpu.pc() < 0x0100_0000 => {
                    Self::bios_read(&self.memory.bios, addr)
                }
                0x0000_0000..=0x0000_3FFF => T::from_u32(self.memory.bios_value),

                // MMIO
                0x0400_0000..=0x04FF_FFFF => match T::WIDTH {
                    1 if addr.is_bit(0) => T::from_u8(self.get_mmio(addr).high()),
                    1 => T::from_u8(self.get_mmio(addr).low()),
                    2 => T::from_u16(self.get_mmio(addr)),
                    4 => T::from_u32(word(self.get_mmio(addr), self.get_mmio(addr + 2))),
                    _ => unreachable!(),
                },

                // Cart save
                // Flash / SRAM
                0x0E00_0000..=0x0FFF_FFFF => {
                    // Reading [half]words causes the byte to be repeated
                    let byte = self.cart.read_ram_byte(addr_unaligned.us() & 0xFFFF);
                    match T::WIDTH {
                        1 => T::from_u8(byte),
                        2 => T::from_u16(hword(byte, byte)),
                        4 => T::from_u32(word(hword(byte, byte), hword(byte, byte))),
                        _ => unreachable!(),
                    }
                }
                // EEPROM
                0x0D00_0000..=0x0DFF_FFFF if T::WIDTH == 2 && self.cart.is_eeprom_at(addr) => {
                    T::from_u16(self.cart.read_ram_hword())
                }

                // Account for unmapped last page due to EEPROM
                0x0DFF_8000..=0x0DFF_FFFF
                    if self.cart.rom.len() >= (addr.us() - 0x800_0001 + T::WIDTH.us()) =>
                unsafe {
                    let ptr = self.cart.rom.as_ptr().add(addr.us() - 0x800_0000);
                    ptr.cast::<T>().read()
                },

                _ if T::WIDTH == 4 => T::from_u32(self.invalid_read::<true>(addr)),
                _ => T::from_u32(self.invalid_read::<false>(addr)),
            }
        })
    }

    fn get_mmio(&self, addr: u32) -> u16 {
        let a = addr & 0x1FFE;
        match a {
            // Timers
            TM0CNT_L => self.timers.time_read(0, self.scheduler.now()),
            TM1CNT_L => self.timers.time_read(1, self.scheduler.now()),
            TM2CNT_L => self.timers.time_read(2, self.scheduler.now()),
            TM3CNT_L => self.timers.time_read(3, self.scheduler.now()),
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
            WAITCNT => self.memory.waitcnt.into(),

            // DMA
            0xBA => Into::<u16>::into(self.dma.channels[0].ctrl) & 0xF7E0,
            0xC6 => Into::<u16>::into(self.dma.channels[1].ctrl) & 0xF7E0,
            0xD2 => Into::<u16>::into(self.dma.channels[2].ctrl) & 0xF7E0,
            0xDE => Into::<u16>::into(self.dma.channels[3].ctrl) & 0xFFE0,
            // DMA length registers read 0
            0xB8 | 0xC4 | 0xD0 | 0xDC => 0,

            // Serial
            RCNT => self.serial.rcnt,

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
            | 0x138..=0x140
            | 0x144..=0x158
            | 0x15C..=0x1FF
            | 0x304..=0x3FE
            | 0x100C => {
                self.c.debugger.log(
                    "invalid-mmio-read-known",
                    format!(
                        "Read from write-only/shadow IO register 0x{a:03X}, returning open bus"
                    ),
                    Severity::Info,
                );
                self.invalid_read::<false>(addr).u16()
            }

            _ => {
                self.c.debugger.log(
                    "invalid-mmio-read-unknown",
                    format!("Read from unknown IO register 0x{a:03X}, returning open bus"),
                    Severity::Warning,
                );
                self.invalid_read::<false>(addr).u16()
            }
        }
    }

    fn invalid_read<const WORD: bool>(&self, addr: u32) -> u32 {
        let shift = (addr & 3) << 3;
        let value = match addr {
            0x0800_0000..=0x0DFF_FFFF => {
                // Out of bounds ROM read
                let addr = (addr & !if WORD { 3 } else { 1 }) >> 1;
                let low = addr.u16();
                return word(low, low.wrapping_add(1));
            }

            _ if self.cpu.pc() == self.dma.pc_at_last_end => self.dma.cache,

            _ => {
                // Open bus
                if self.cpu.pc() > 0xFFF_FFFF
                    || (self.cpu.pc() > 0x3FFF && self.cpu.pc() < 0x200_0000)
                    || (self.cpu.pc() >= 0x400_0000 && self.cpu.pc() < 0x500_0000)
                {
                    return 0;
                }

                if !self.cpu.flag(Flag::Thumb) {
                    // Simple case: just read PC in ARM mode
                    self.get(self.cpu.pc())
                } else {
                    // Thumb mode... complicated.
                    // https://problemkaputt.de/gbatek.htm#gbaunpredictablethings
                    match self.cpu.pc() >> 24 {
                        0x02 | 0x05 | 0x06 | 0x08..=0xD => {
                            let hword = self.get(self.cpu.pc());
                            word(hword, hword)
                        }
                        _ if self.cpu.pc().is_bit(1) => {
                            word(self.get(self.cpu.pc() - 2), self.get(self.cpu.pc()))
                        }
                        0x00 | 0x07 => word(self.get(self.cpu.pc()), self.get(self.cpu.pc() + 2)),
                        0x03 => word(self.get(self.cpu.pc()), self.get(self.cpu.pc() - 2)),

                        _ => unreachable!(),
                    }
                }
            }
        };
        value >> shift
    }

    /// Write a byte to the bus. Does no timing-related things; simply sets the
    /// value.
    pub fn set<T: RwType>(&mut self, addr_unaligned: u32, value: T) {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        if addr >= 0x1000_0000 {
            return;
        }

        // Bytes only use the mapper later, since VRAM does weird behavior
        // on byte writes
        if T::WIDTH != 1 {
            let success = self.memory.mapper.set::<Self, _>(addr, value);
            if success {
                self.cpu.cache.write(addr);
                return;
            }
        }

        match addr {
            // MMIO
            0x0400_0000..=0x0400_0301 if T::WIDTH == 1 => self.set_mmio_byte(addr, value.u8()),
            0x0400_0000..=0x0400_0300 if T::WIDTH == 2 => self.set_mmio(addr, value.u16()),
            0x0400_0000..=0x0400_0300 if T::WIDTH == 4 => {
                self.set_mmio(addr, value.u16());
                self.set_mmio(addr + 2, value.u32().high());
            }

            // Maybe write EEPROM
            0x0D00_0000..=0x0DFF_FFFF if T::WIDTH == 2 && self.cart.is_eeprom_at(addr) => {
                self.cart.write_ram_hword(value.u16());
            }

            // Other saves
            0x0E00_0000..=0x0FFF_FFFF => {
                let byte = match T::WIDTH {
                    1 => value.u8(),
                    2 if addr_unaligned.is_bit(0) => value.u16().high(),
                    2 => value.u8(),
                    4 => {
                        let byte_shift = (addr_unaligned & 3) * 8;
                        (value.u32() >> byte_shift).u8()
                    }
                    _ => unreachable!(),
                };
                self.cart.write_ram_byte(addr_unaligned.us() & 0xFFFF, byte);
            }

            // VRAM weirdness on byte writes
            _ if T::WIDTH == 1 => {
                let value = value.u8();
                match addr {
                    0x0500_0000..=0x0600_FFFF if !self.ppu.regs.is_bitmap_mode() => {
                        self.set(addr & !1, hword(value, value))
                    }
                    0x0500_0000..=0x0601_3FFF => self.set(addr & !1, hword(value, value)),
                    0x0602_0000..=0x06FF_FFFF if addr & 0x1_FFFF < 0x1_0000 => {
                        // Only BG VRAM gets written to, OBJ VRAM is ignored
                        self.set(addr & !1, hword(value, value));
                    }
                    0x0601_0000..=0x07FF_FFFF if !self.ppu.regs.is_bitmap_mode() => (), // Ignored
                    0x0601_4000..=0x07FF_FFFF => (),                                    // Ignored

                    _ => {
                        self.memory.mapper.set::<Self, _>(addr, value);
                    }
                };
            }

            _ => (),
        }
        self.cpu.cache.write(addr);
    }

    fn set_mmio_byte(&mut self, addr: u32, value: u8) {
        let a = addr & 0x3FF;
        match a {
            // DMA channel edge case, why do games do this
            0xA0..=0xA3 => self.apu.push_sample::<0>(value),
            0xA4..=0xA7 => self.apu.push_sample::<1>(value),

            // Control registers
            0x301 => self.cpu.halt_on_irq(),
            WAITCNT => self.set_mmio(
                addr,
                hword(value, Into::<u16>::into(self.memory.waitcnt).high()),
            ),
            0x205 => self.set_mmio(
                addr,
                hword(Into::<u16>::into(self.memory.waitcnt).low(), value),
            ),

            // Old sound
            0x60..=0x80 | 0x84 | 0x90..=0x9F => {
                Apu::write_register_psg(
                    &mut self.apu.cgb_chans,
                    (addr & 0xFFF).u16(),
                    value,
                    &mut audio::shed(&mut self.scheduler),
                );
            }

            // DMAs
            0xB0..0xE0 => {
                let idx = (a.us() - 0xB0) / 12;
                let reg = (a - 0xB0) % 12;

                let dma = &mut self.dma.channels[idx];
                let ctrl = Into::<u16>::into(dma.ctrl);
                match reg {
                    0x0 => dma.sad = dma.sad.set_low(dma.sad.low().set_low(value)),
                    0x1 => dma.sad = dma.sad.set_low(dma.sad.low().set_high(value)),
                    0x2 => dma.sad = dma.sad.set_high(dma.sad.high().set_low(value)),
                    0x3 => dma.sad = dma.sad.set_high(dma.sad.high().set_high(value)),
                    0x4 => dma.sad = dma.dad.set_low(dma.dad.low().set_low(value)),
                    0x5 => dma.sad = dma.dad.set_low(dma.dad.low().set_high(value)),
                    0x6 => dma.sad = dma.dad.set_high(dma.dad.high().set_low(value)),
                    0x7 => dma.sad = dma.dad.set_high(dma.dad.high().set_high(value)),
                    0x8 => dma.count = dma.count.set_low(value),
                    0x9 => dma.count = dma.count.set_high(value),
                    0xA => Dmas::ctrl_write(self, idx, ctrl.set_low(value)),
                    0xB => Dmas::ctrl_write(self, idx, ctrl.set_high(value)),
                    _ => unreachable!(),
                }
            }

            // PPU
            DISPCNT..=BLDY => self.ppu.regs.write_mmio_byte(a, value),

            // Treat as halfword
            _ if addr.is_bit(0) => {
                self.set::<u16>(addr, Self::get::<u16>(self, addr).set_high(value));
            }
            _ => self.set::<u16>(addr, Self::get::<u16>(self, addr).set_low(value)),
        }
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
                let prev: u16 = self.memory.waitcnt.into();
                self.memory.waitcnt = value.into();
                // Only update things as needed
                if value.bits(0, 11) != prev.bits(0, 11) {
                    self.update_wait_times();
                }
                if value.bit(14) != prev.bit(14) {
                    self.memory.prefetch.active = false;
                    self.memory.prefetch.restart = true;
                    self.cpu.cache.invalidate_rom();
                } else if value.bits(2, 9) != prev.bits(2, 9) {
                    self.cpu.cache.invalidate_rom();
                }
            }

            // DMA Audio
            FIFO_A_L | FIFO_A_H => self.apu.push_samples::<0>(value),
            FIFO_B_L | FIFO_B_H => self.apu.push_samples::<1>(value),
            SOUNDCNT_H => self.apu.cnt = value.into(),
            SOUNDBIAS_L => {
                self.apu.bias = value.into();
                // TODO update sample rate
            }

            // PPU
            DISPCNT..=BLDY => self.ppu.regs.write_mmio(a, value),

            // Timers
            TM0CNT_L => self.timers.reload[0] = value,
            TM1CNT_L => self.timers.reload[1] = value,
            TM2CNT_L => self.timers.reload[2] = value,
            TM3CNT_L => self.timers.reload[3] = value,
            TM0CNT_H => self.timers.hi_write(&mut self.scheduler, 0, value),
            TM1CNT_H => self.timers.hi_write(&mut self.scheduler, 1, value),
            TM2CNT_H => self.timers.hi_write(&mut self.scheduler, 2, value),
            TM3CNT_H => self.timers.hi_write(&mut self.scheduler, 3, value),

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
            RCNT => self.serial.rcnt = (self.serial.rcnt & 0x800F) | (value & 0x41F0),

            // RO registers, or otherwise invalid
            KEYINPUT
            | 0x86
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
            | 0x20A..=0x21E => self.c.debugger.log(
                "invalid-mmio-write-known",
                format!(
                    "Write to known read-only IO register 0x{a:03X} (value {value:04X}), ignoring"
                ),
                Severity::Info,
            ),

            _ => self.c.debugger.log(
                "invalid-mmio-write-unknown",
                format!("Write to unknown IO register 0x{a:03X} (value {value:04X}), ignoring"),
                Severity::Warning,
            ),
        }
    }

    fn bios_read<T>(bios: &[u8], addr: u32) -> T {
        unsafe {
            let ptr = bios.as_ptr().add(addr.us() & 0x3FFF);
            ptr.cast::<T>().read()
        }
    }

    /// Get wait time for a given address.
    #[inline]
    pub fn wait_time<T: NumExt + 'static>(&mut self, addr: u32, ty: Access) -> u16 {
        let region = addr.us() >> 24;
        let wait = self.wait_time_inner::<T>(addr, ty);
        match region {
            0x08..=0x0D => self.handle_prefetch::<T>(addr, ty, wait),
            0x0E..=0x0F => {
                self.stop_prefetch();
                wait
            }
            0x10.. => 1,
            _ => wait,
        }
    }

    fn handle_prefetch<T: NumExt + 'static>(
        &mut self,
        addr: u32,
        ty: Access,
        mut regular: u16,
    ) -> u16 {
        if (ty & CODE) == 0 {
            self.stop_prefetch();
            return regular;
        }

        let pf = &mut self.memory.prefetch;
        if pf.active {
            // Value is head of buffer
            if pf.count != 0 && addr == pf.head {
                pf.count -= 1;
                pf.head += T::WIDTH;
                return 1;
            }
            // Value is being prefetched
            if pf.countdown > 0 && addr == pf.tail {
                pf.head = pf.tail;
                pf.count = 0;
                return pf.countdown as u16;
            }
        }

        self.stop_prefetch();

        // Prefetch should keep transfer alive
        if self.memory.waitcnt.prefetch_en() {
            let duty = if self.cpu.flag(Flag::Thumb) {
                self.wait_time_inner::<u16>(addr, SEQ | CODE)
            } else {
                self.wait_time_inner::<u32>(addr, SEQ | CODE)
            };

            let pf = &mut self.memory.prefetch;
            if pf.restart {
                pf.restart = false;
                // Force non-seq
                regular = self.wait_time_inner::<T>(addr, CODE);
            }

            let pf = &mut self.memory.prefetch;
            pf.thumb = self.cpu.flag(Flag::Thumb);
            pf.tail = addr + T::WIDTH;
            pf.head = pf.tail;
            pf.active = true;
            pf.count = 0;
            pf.duty = duty;
            pf.countdown = duty as i16;
        }

        regular
    }

    pub(super) fn step_prefetch(&mut self, count: u16) {
        let pf = &mut self.memory.prefetch;
        if pf.active {
            pf.countdown -= count as i16;
            while pf.countdown <= 0 {
                let capacity = if pf.thumb { 8 } else { 4 };
                let size = if pf.thumb { 2 } else { 4 };
                pf.countdown += pf.duty as i16;
                if self.memory.waitcnt.prefetch_en() && pf.count < capacity {
                    pf.count += 1;
                    pf.tail += size;
                }
            }
        }
    }

    pub(super) fn stop_prefetch(&mut self) {
        let prefetch = &mut self.memory.prefetch;
        if prefetch.active {
            // Penalty for accessing ROM/RAM during last cycle of prefetch fetch
            if self.cpu.pc() >= 0x800_0000 && self.cpu.pc() < 0xE00_0000 {
                let duty = prefetch.duty / 2 + 1;
                if prefetch.countdown == 1 || (!prefetch.thumb && duty == prefetch.countdown as u16)
                {
                    self.add_i_cycles(1);
                    self.cpu().access_type = NONSEQ;
                }
            }
            self.memory.prefetch.active = false;
        }
    }

    fn wait_time_inner<T: NumExt + 'static>(&mut self, addr: u32, ty: Access) -> u16 {
        let region = (addr.us() >> 24) & 0xF;
        let ty_idx = if ty & SEQ != 0 { 16 } else { 0 };
        if T::WIDTH == 4 {
            self.memory.wait_word[region + ty_idx]
        } else {
            self.memory.wait_other[region + ty_idx]
        }
    }

    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {
        MemoryMapper::init_pages(self);
        self.update_wait_times();
        if self.c.config.cached_interpreter {
            self.cpu.cache.init(self.cart.rom.len());
        }
    }

    fn update_wait_times(&mut self) {
        for i in 0..16 {
            let addr = i.u32() * 0x100_0000;
            self.memory.wait_word[i] = self.calc_wait_time::<4>(addr, NONSEQ);
            self.memory.wait_other[i] = self.calc_wait_time::<2>(addr, NONSEQ);
            self.memory.wait_word[i + 16] = self.calc_wait_time::<4>(addr, SEQ);
            self.memory.wait_other[i + 16] = self.calc_wait_time::<2>(addr, SEQ);
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
                self.calc_wait_time::<2>(addr, ty) + self.calc_wait_time::<2>(addr, SEQ)
            }

            (0x0800_0000..=0x09FF_FFFF, _, SEQ) => 3 - self.memory.waitcnt.ws0_s().u16(),
            (0x0800_0000..=0x09FF_FFFF, _, NONSEQ) => {
                Self::WS_NONSEQ[self.memory.waitcnt.ws0_n().us()]
            }

            (0x0A00_0000..=0x0BFF_FFFF, _, SEQ) => 5 - (self.memory.waitcnt.ws1_s().u16() * 3),
            (0x0A00_0000..=0x0BFF_FFFF, _, NONSEQ) => {
                Self::WS_NONSEQ[self.memory.waitcnt.ws1_n().us()]
            }

            (0x0C00_0000..=0x0DFF_FFFF, _, SEQ) => 9 - (self.memory.waitcnt.ws2_s().u16() * 7),
            (0x0C00_0000..=0x0DFF_FFFF, _, NONSEQ) => {
                Self::WS_NONSEQ[self.memory.waitcnt.ws2_n().us()]
            }

            (0x0E00_0000..=0x0EFF_FFFF, _, _) => Self::WS_NONSEQ[self.memory.waitcnt.sram().us()],

            _ => 1,
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            bios: BIOS.into(),
            ewram: Box::new([0; 256 * KB]),
            iwram: Box::new([0; 32 * KB]),
            keycnt: 0.into(),
            keys_prev: 0,
            waitcnt: 0.into(),
            bios_value: 0xE129_F000,
            mapper: MemoryMapper::default(),
            prefetch: Prefetch::default(),
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

        // 1MB ROMs (Classic NES) mirror
        let rom_1mb = self.cart.rom.len() == (2 << 19);
        match a {
            0x0200_0000..=0x02FF_FFFF => offs(&self.memory.ewram, a - 0x200_0000),
            0x0300_0000..=0x03FF_FFFF => offs(&self.memory.iwram, a - 0x300_0000),
            0x0500_0000..=0x05FF_FFFF => offs(&self.ppu.palette, a - 0x500_0000),
            0x0600_0000..=0x0601_7FFF => offs(&self.ppu.vram, a - 0x600_0000),
            0x0700_0000..=0x07FF_FFFF => offs(&self.ppu.oam, a - 0x700_0000),
            0x0800_0000..=0x09FF_FFFF
                if R && (self.cart.rom.len() >= (a - 0x800_0000) || rom_1mb) =>
            {
                offs(&self.cart.rom, a - 0x800_0000)
            }
            0x0A00_0000..=0x0BFF_FFFF
                if R && (self.cart.rom.len() >= (a - 0x800_0000) || rom_1mb) =>
            {
                offs(&self.cart.rom, a - 0xA00_0000)
            }
            // Does not go all the way due to EEPROM
            0x0C00_0000..=0x0DFF_7FFF
                if R && (self.cart.rom.len() >= (a - 0x800_0000) || rom_1mb) =>
            {
                offs(&self.cart.rom, a - 0xC00_0000)
            }

            // VRAM mirror weirdness
            0x0601_8000..=0x0601_FFFF => offs(&self.ppu.vram, 0x1_0000 + (a - 0x600_0000)),
            0x0602_0000..=0x06FF_FFFF => self.get_page::<R>(a & 0x601_FFFF),
            _ => ptr::null::<u8>() as *mut u8,
        }
    }
}
