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
    TimeS,
};

use super::GameGirl;
use crate::io::{
    addr::*,
    apu::Apu,
    cartridge::Cartridge,
    dma::Hdma,
    scheduling::{GGEvent, PpuEvent},
    timer::Timer,
};

pub mod addr;
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub vram: [u8; 2 * 8192],
    vram_bank: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    wram: [u8; 4 * 8192],
    wram_bank: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    oam: [u8; 160],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    high: [u8; 256],
    pending_dma: Option<u64>,
    dma_restarted: bool,

    mapper: MemoryMapper<256>,
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    page_offsets: [u32; 16],

    pub(super) bootrom_enable: bool,
}

impl GameGirl {
    pub fn read8(&mut self, addr: u16) -> u8 {
        self.advance_clock(1);
        self.get(addr)
    }

    pub fn read_s8(&mut self, addr: u16) -> i8 {
        self.read8(addr) as i8
    }

    pub fn write8(&mut self, addr: u16, value: u8) {
        self.debugger.write_occurred(addr);

        // TODO Hack to pass another mooneye test
        if addr == (TMA + 0xFF00) {
            self.set(addr, value);
        }

        self.advance_clock(1);
        self.set(addr, value);
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
        self.read8(self.cpu.pc.wrapping_add(1))
    }

    /// Get a 16-bit argument for the current CPU instruction.
    pub fn arg16(&mut self) -> u16 {
        self.read16(self.cpu.pc.wrapping_add(1))
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

    pub fn get<T: NumExt>(&self, addr: u16) -> T {
        if T::WIDTH == 2 {
            let low = self.get::<u8>(addr);
            let high = self.get::<u8>(addr.wrapping_add(1));
            return T::from_u16(hword(low, high));
        }

        T::from_u8(self.get_inner(addr, |this, addr| {
            match addr {
                0xA000..=0xBFFF => this.cart.read(addr),
                0xFE00..=0xFE9F
                    if !this
                        .mem
                        .pending_dma
                        .is_some_and(|t| t != (this.scheduler.now() - 4))
                        && !this.mem.dma_restarted =>
                {
                    this.mem.oam[addr.us() & 0xFF]
                }
                0xFF00..=0xFFFF => this.get_high(addr & 0x00FF),
                _ => 0xFF,
            }
        }))
    }

    pub fn set(&mut self, addr: u16, value: u8) {
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
            0xC000..=0xCFFF => self.mem.wram[a & 0x0FFF] = value,
            0xD000..=0xDFFF => {
                self.mem.wram[(a & 0x0FFF) + (self.mem.wram_bank.us() * 0x1000)] = value;
            }
            0xE000..=0xFDFF => self.mem.wram[a & 0x1FFF] = value,
            0xFE00..=0xFE9F => self.mem.oam[a & 0xFF] = value,
            0xFF00..=0xFFFF => self.set_high(addr & 0x00FF, value),
            _ => (),
        }
    }

    fn get_high(&self, addr: u16) -> u8 {
        match addr {
            DMA => self.dma,
            JOYP => self.joypad.read(self[JOYP]),
            DIV => Timer::read(self, addr),

            LY if !self[LCDC].is_bit(7) => 0,
            BCPS..=OCPD => self.ppu.read_high(addr),

            NR10..=WAV_END => self.apu.read_register_gg(HIGH_START + addr),
            0x76 if self.cgb => self.apu.read_pcm12(),
            0x77 if self.cgb => self.apu.read_pcm34(),

            VRAM_SELECT if self.cgb => self.mem.vram_bank | 0xFE,
            WRAM_SELECT if self.cgb => self.mem.wram_bank | 0xF8,
            HDMA_SRC_HIGH..=HDMA_START if self.cgb => self.hdma.get(addr),

            _ => self[addr],
        }
    }

    fn set_high(&mut self, addr: u16, value: u8) {
        match addr {
            IF => self[IF] = value | 0xE0,
            IE => self[IE] = value,
            BOOTROM_DISABLE => {
                self.mem.bootrom_enable = false;
                // Refresh page tables
                MemoryMapper::init_pages(self);
            }

            DIV..=TAC => Timer::write(self, addr, value),
            LCDC => {
                let was_on = self[LCDC].is_bit(7);
                let is_on = value.is_bit(7);
                self[LCDC] = value;

                if !is_on {
                    self[STAT] &= 0xF8;
                }
                if was_on && !is_on {
                    let time = self
                        .scheduler
                        .cancel_with_remaining(|e| matches!(e, GGEvent::PpuEvent(_)));
                    self.ppu.resume_data = Some(time);
                }
                if !was_on && is_on {
                    let data = self.ppu.resume_data.take();
                    if let Some(data) = data {
                        self.scheduler.schedule(data.1, data.0 as TimeS);
                    }
                }
            }
            STAT => self[STAT] = value | 0x80, // Bit 7 unavailable
            DMA => dma::dma_written(self, value),
            BCPS..=OPRI => self.ppu.write_high(addr, value),
            NR10..=WAV_END => self.apu.write_register_gg(HIGH_START + addr, value),

            SB => self.debugger.serial_output.push(value as char),

            VRAM_SELECT if self.cgb => {
                self.mem.vram_bank = value & 1;
                self.mem.page_offsets[8] = self.mem.vram_bank.u32() * 0x2000;
                self.mem.page_offsets[9] = self.mem.vram_bank.u32() * 0x2000;
            }
            WRAM_SELECT if self.cgb => {
                self.mem.wram_bank = u8::max(1, value & 7);
                self.mem.page_offsets[0xD] = self.mem.wram_bank.u32() * 0x1000;
            }
            KEY1 if self.cgb => self[KEY1] = (value & 1) | (self[KEY1] & 0x80),
            HDMA_SRC_HIGH..=HDMA_START if self.cgb => Hdma::set(self, addr, value),

            // Last 3 are unmapped regions.
            KEY1 | LY | SC | 0x03 | 0x08..=0x0E | 0x4C..=0x7F => (),
            _ => self[addr] = value,
        }
    }

    pub(super) fn load_cart_mem(&mut self, cart: Cartridge, conf: &SystemConfig) {
        self.cgb = match conf.mode {
            CgbMode::Always => true,
            CgbMode::Prefer => cart.supports_cgb(),
            CgbMode::Never => cart.requires_cgb(),
        };
        self.ppu.configure(self.cgb, conf.cgb_colour_correction);
        self.apu = Apu::new(!self.cgb);
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
        self[TAC] = 0xF8;
        self[TIMA] = 0;
        self[TMA] = 0;
        self[IF] = 0xE0;
        self[IE] = 0xE0;
        self.set(0xFF82, 0x8F);

        if self.cgb {
            self[KEY1] = 0;
        }
    }

    fn init_scheduler(&mut self) {
        self.scheduler
            .schedule(GGEvent::PpuEvent(PpuEvent::OamScanEnd), 80);
    }

    // Unsafe corner!
    /// Get a value in memory. Will try to do a fast read from page tables,
    /// falls back to given closure if no page table is mapped at that address.
    #[inline]
    fn get_inner<T>(&self, a: u16, slow: fn(&Self, u16) -> T) -> T {
        let ptr = unsafe {
            self.mem
                .mapper
                .page::<Self, false>(a)
                .add(self.mem.page_offsets.get_unchecked(a.us() >> 12).us())
        };

        if ptr as usize > 0xFFFF {
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
            pending_dma: None,
            dma_restarted: false,
            high: [0xFF; 256],

            mapper: MemoryMapper::default(),
            page_offsets: [
                0, 0, 0, 0, 0x4000, 0x4000, 0x4000, 0x4000, 0, 0, 0, 0, 0, 0x1000, 0, 0,
            ],

            bootrom_enable: true,
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
            0x0000..=0x00FF if self.mem.bootrom_enable && self.cgb => offs(CGB_BOOTROM, a),
            0x0000..=0x00FF if self.mem.bootrom_enable => offs(BOOTIX_ROM, a),
            0x0200..=0x08FF if self.mem.bootrom_enable && self.cgb => offs(CGB_BOOTROM, a - 0x0100),
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
