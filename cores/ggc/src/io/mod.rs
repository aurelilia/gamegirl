// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    ops::{Index, IndexMut},
    ptr,
};

use common::{
    components::memory::{MemoryMappedSystem, MemoryMapper},
    misc::{CgbMode, SystemConfig},
    numutil::{hword, NumExt},
};
use serde::{Deserialize, Serialize};

use super::GameGirl;
use crate::io::{
    addr::*,
    apu::Apu,
    cartridge::Cartridge,
    dma::Hdma,
    scheduling::{GGEvent, PpuEvent},
    timer::Timer,
};

pub(super) mod addr;
pub mod apu;
pub mod cartridge;
pub mod dma;
pub mod joypad;
pub mod ppu;
pub mod scheduling;
pub mod timer;

/// The memory of the GG, containing big arrays holding internal memory.
///
/// IO registers can be directly read by IO devices by indexing the GG,
/// the various addresses are defined in the `addr` submodule.
#[derive(Deserialize, Serialize)]
pub struct Memory {
    #[serde(with = "serde_arrays")]
    pub vram: [u8; 2 * 8192],
    vram_bank: u8,
    #[serde(with = "serde_arrays")]
    wram: [u8; 4 * 8192],
    wram_bank: u8,
    #[serde(with = "serde_arrays")]
    oam: [u8; 160],
    #[serde(with = "serde_arrays")]
    high: [u8; 256],
    dma_active: bool,

    mapper: MemoryMapper<256>,
    #[serde(with = "serde_arrays")]
    page_offsets: [u32; 16],

    #[serde(skip)]
    #[serde(default)]
    pub(super) bootrom: Option<Vec<u8>>,
}

impl GameGirl {
    pub fn read8(&mut self, addr: u16) -> u8 {
        self.advance_clock(1);
        self.get8(addr)
    }

    pub fn read_s8(&mut self, addr: u16) -> i8 {
        self.read8(addr) as i8
    }

    pub fn write8(&mut self, addr: u16, value: u8) {
        self.options.running &= self.debugger.write_occurred(addr);
        self.advance_clock(1);
        self.set8(addr, value);
    }

    pub fn read16(&mut self, addr: u16) -> u16 {
        let low = self.read8(addr);
        let high = self.read8(addr.wrapping_add(1));
        (high.u16() << 8) | low.u16()
    }

    pub fn write16(&mut self, addr: u16, value: u16) {
        self.write8(addr.wrapping_add(1), (value >> 8).u8());
        self.write8(addr, value.u8());
    }

    /// Get an 8-bit argument for the current CPU instruction.
    pub fn arg8(&mut self) -> u8 {
        self.read8(self.cpu.pc + 1)
    }

    /// Get a 16-bit argument for the current CPU instruction.
    pub fn arg16(&mut self) -> u16 {
        self.read16(self.cpu.pc + 1)
    }

    /// Pop the current value off the SP.
    pub fn pop_stack(&mut self) -> u16 {
        let val = self.read16(self.cpu.sp);
        self.cpu.sp = self.cpu.sp.wrapping_add(2);
        val
    }

    /// Push the given value to the current SP.
    pub fn push_stack(&mut self, value: u16) {
        self.cpu.sp = self.cpu.sp.wrapping_sub(2);
        self.write16(self.cpu.sp, value);
    }

    pub fn get8(&self, addr: u16) -> u8 {
        self.get(addr, |this, addr| match addr {
            0xA000..=0xBFFF => this.cart.read(addr),
            0xFE00..=0xFE9F if !this.mem.dma_active => this.mem.oam[addr.us() & 0xFF],
            0xFF00..=0xFFFF => this.get_high(addr & 0x00FF),
            _ => 0xFF,
        })
    }

    pub fn get16(&self, addr: u16) -> u16 {
        let low = self.get8(addr);
        let high = self.get8(addr.wrapping_add(1));
        hword(low, high)
    }

    fn get_high(&self, addr: u16) -> u8 {
        match addr {
            JOYP => self.joypad.read(self[JOYP]),
            DIV | TIMA | TAC => Timer::read(self, addr),

            LY if !self[LCDC].is_bit(7) => 0,
            BCPS..=OCPD => self.ppu.read_high(addr),

            NR10..=WAV_END => Apu::read_register(&self.apu.inner, HIGH_START + addr),
            0x76 if self.cgb => Apu::read_pcm12(&self.apu.inner),
            0x77 if self.cgb => Apu::read_pcm34(&self.apu.inner),

            HDMA_START if self.cgb => self.hdma.transfer_left as u8,
            HDMA_SRC_HIGH..=HDMA_DEST_LOW => 0xFF,

            _ => self[addr],
        }
    }

    pub fn set8(&mut self, addr: u16, value: u8) {
        let a = addr.us();
        match addr {
            0x0000..=0x7FFF => {
                self.cart.write(addr, value);
                // Refresh page offsets
                for i in 0..4 {
                    self.mem.page_offsets[i] = self.cart.rom0_bank.u32() * 0x4000;
                }
                for i in 4..8 {
                    self.mem.page_offsets[i] = self.cart.rom1_bank.u32() * 0x4000;
                }
            }
            0xA000..=0xBFFF => self.cart.write(addr, value),
            0x8000..=0x9FFF => {
                self.mem.vram[(a & 0x1FFF) + (self.mem.vram_bank.us() * 0x2000)] = value;
            }
            0xC000..=0xCFFF => self.mem.wram[(a & 0x0FFF)] = value,
            0xD000..=0xDFFF => {
                self.mem.wram[(a & 0x0FFF) + (self.mem.wram_bank.us() * 0x1000)] = value;
            }
            0xE000..=0xFDFF => self.mem.wram[a & 0x1FFF] = value,
            0xFE00..=0xFE9F => self.mem.oam[a & 0xFF] = value,
            0xFF00..=0xFFFF => self.set_high(addr & 0x00FF, value),
            _ => (),
        }
    }

    fn set_high(&mut self, addr: u16, value: u8) {
        match addr {
            VRAM_SELECT if self.cgb => {
                self.mem.vram_bank = value & 1;
                self.mem.page_offsets[8] = self.mem.vram_bank.u32() * 0x2000;
                self.mem.page_offsets[9] = self.mem.vram_bank.u32() * 0x2000;
                self[VRAM_SELECT] = value | 0xFE;
            }
            WRAM_SELECT if self.cgb => {
                self.mem.wram_bank = u8::max(1, value & 7);
                self.mem.page_offsets[0xD] = self.mem.wram_bank.u32() * 0x1000;
                self[WRAM_SELECT] = value | 0xF8;
            }
            KEY1 if self.cgb => self[KEY1] = (value & 1) | self[KEY1] & 0x80,
            HDMA_START if self.cgb => Hdma::write_start(self, value),
            HDMA_SRC_HIGH..=HDMA_DEST_LOW if self.cgb => self[addr] = value,

            IF => self[IF] = value | 0xE0,
            IE => self[IE] = value,
            BOOTROM_DISABLE => {
                self.mem.bootrom = None;
                // Refresh page tables
                MemoryMapper::init_pages(self);
            }

            DIV | TIMA | TAC => Timer::write(self, addr, value),
            LCDC => {
                self[LCDC] = value;
                if !value.is_bit(7) {
                    self[STAT] &= 0xF8;
                }
            }
            STAT => self[STAT] = value | 0x80, // Bit 7 unavailable
            DMA => {
                self[addr] = value;
                let time = 648 / self.speed as i32;
                self.scheduler.cancel(GGEvent::DMAFinish);
                self.scheduler.schedule(GGEvent::DMAFinish, time);
                self.mem.dma_active = true;
            }
            BCPS..=OPRI => self.ppu.write_high(addr, value),
            NR10..=WAV_END => Apu::write(self, HIGH_START + addr, value),

            SB => self.debugger.serial_output.push(value as char),

            // Last 3 are unmapped regions.
            LY | SC | 0x03 | 0x08..=0x0E | 0x4C..=0x7F => (),
            _ => self[addr] = value,
        }
    }

    pub(super) fn load_cart_mem(&mut self, cart: Cartridge, conf: &SystemConfig) {
        self.cgb = match conf.mode {
            CgbMode::Always => true,
            CgbMode::Prefer => cart.supports_cgb(),
            CgbMode::Never => cart.requires_cgb(),
        };
        self.mem.bootrom = Some(if self.cgb {
            CGB_BOOTROM.to_vec()
        } else {
            BOOTIX_ROM.to_vec()
        });
        self.ppu.configure(self.cgb, conf.cgb_colour_correction);
        self.apu = Apu::new(self.cgb);
        self.cart = cart;
        MemoryMapper::init_pages(self);
        self.init_high();
        self.init_scheduler();
    }

    fn init_high(&mut self) {
        self[LY] = 0;
        self[LYC] = 0;
        self[LCDC] = 0;
        self[STAT] = 0x80;
        self[SCY] = 0;
        self[SCX] = 0;
        self[WY] = 0;
        self[WX] = 0;
        self[BGP] = 0b1110_0100;
        self[OBP0] = 0b1110_0100;
        self[OBP1] = 0b1110_0100;

        self[SB] = 0;
        self[SC] = 0x7E;
        self[TIMA] = 0;
        self[TMA] = 0;
        self[IF] = 0xE0;

        if self.cgb {
            self[KEY1] = 0;
        }
    }

    fn init_scheduler(&mut self) {
        self.scheduler
            .schedule(GGEvent::PpuEvent(PpuEvent::OamScanEnd), 80);
        Apu::init_scheduler(self);
    }

    // Unsafe corner!
    /// Get a value in memory. Will try to do a fast read from page tables,
    /// falls back to given closure if no page table is mapped at that address.
    #[inline]
    fn get<T>(&self, a: u16, slow: fn(&Self, u16) -> T) -> T {
        let ptr = unsafe {
            self.mem
                .mapper
                .page::<Self, false>(a)
                .add(self.mem.page_offsets.get_unchecked(a.us() >> 12).us())
        };

        if ptr as usize > 0xFF_FFFF {
            unsafe { (ptr as *const T).read() }
        } else {
            slow(self, a)
        }
    }
}

impl Memory {
    pub(super) fn new() -> Self {
        Self {
            vram: [0; 16384],
            vram_bank: 0,
            wram: [0; 32768],
            wram_bank: 1,
            oam: [0; 160],
            dma_active: false,
            high: [0xFF; 256],

            mapper: MemoryMapper::default(),
            page_offsets: [
                0, 0, 0, 0, 0x4000, 0x4000, 0x4000, 0x4000, 0, 0, 0, 0, 0, 0x1000, 0, 0,
            ],

            bootrom: None,
        }
    }
}

impl Index<u16> for GameGirl {
    type Output = u8;
    fn index(&self, index: u16) -> &Self::Output {
        &self.mem.high[index.us()]
    }
}

impl IndexMut<u16> for GameGirl {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        &mut self.mem.high[index.us()]
    }
}

unsafe impl Send for Memory {}

impl MemoryMappedSystem<256> for GameGirl {
    type Usize = u16;
    const ADDR_MASK: &'static [usize] = &[0xFF];
    const PAGE_POW: usize = 8;
    const MASK_POW: usize = 0;

    fn get_mapper(&self) -> &MemoryMapper<256> {
        &self.mem.mapper
    }

    fn get_mapper_mut(&mut self) -> &mut MemoryMapper<256> {
        &mut self.mem.mapper
    }

    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8 {
        unsafe fn offs(reg: &[u8], offs: usize) -> *mut u8 {
            let ptr = reg.as_ptr() as *mut u8;
            ptr.add(offs)
        }

        if !R {
            return ptr::null::<u8>() as *mut u8;
        }

        match a {
            0x0000..=0x00FF if self.mem.bootrom.is_some() => {
                offs(self.mem.bootrom.as_ref().unwrap(), a)
            }
            0x0200..=0x08FF if self.mem.bootrom.is_some() && self.cgb => {
                offs(self.mem.bootrom.as_ref().unwrap(), a - 0x0100)
            }
            0x0000..=0x3FFF => offs(&self.cart.rom, a),
            0x4000..=0x7FFF => offs(&self.cart.rom, a - 0x4000),

            0x8000..=0x9FFF => offs(&self.mem.vram, a - 0x8000),
            0xC000..=0xCFFF => offs(&self.mem.wram, a - 0xC000),
            0xD000..=0xDFFF => offs(&self.mem.wram, a - 0xD000),
            0xE000..=0xFDFF => offs(&self.mem.wram, a - 0xE000),

            _ => ptr::null::<u8>() as *mut u8,
        }
    }
}
