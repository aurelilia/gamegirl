use crate::numutil::NumExt;
use crate::system::io::addr::*;
use crate::system::io::cartridge::Cartridge;
use crate::system::io::timer::Timer;
use crate::GameGirl;
use std::ops::{Index, IndexMut};

pub mod addr;
mod apu;
mod cartridge;
mod dma;
mod joypad;
mod ppu;
mod timer;

pub struct Mmu {
    pub vram: [u8; 2 * 8192],
    pub vram_bank: u8,
    pub wram: [u8; 2 * 8192],
    pub wram_bank: u8,
    pub oam: [u8; 160],
    pub high: [u8; 256],

    pub bootrom: Option<&'static [u8]>,
    pub cgb: bool,

    pub cart: Cartridge,
    pub timer: Timer,
}

impl Mmu {
    pub fn step(gg: &mut GameGirl, t_cycles: usize) {
        let cycles = t_cycles / gg.t_multiplier.us();
        Timer::step(gg, cycles);
    }

    pub fn read(&self, addr: u16) -> u8 {
        let a = addr.us();
        match addr {
            0x0000..0x0900 if self.bootrom.is_some() => self.bootrom.unwrap()[a],
            0x0000..0x7FFF | 0xA000..0xBFFF => self.cart.read(addr),

            0x8000..0x9FFF => self.vram[(a & 0x1FFF) + (self.vram_bank.us() * 0x2000)],
            0xC000..0xCFFF => self.wram[(a & 0x0FFF)],
            0xD000..0xDFFF => self.wram[(a & 0x0FFF) + (self.wram_bank.us() * 0x1000)],
            0xE000..0xFDFF => self.wram[a & 0x1FFF],
            0xFE00..0xFE9F => self.oam[a & 0xFF],

            _ => self.read_high(addr & 0x00FF),
        }
    }

    fn read_high(&self, addr: u16) -> u8 {
        match addr {
            // TODO: joypad
            DIV | TAC => self.timer.read(addr),
            SB => 0,
            SC => 0x7E,
            _ => self[addr],
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        let a = addr.us();
        match addr {
            0x0000..0x7FFF | 0xA000..0xBFFF => self.cart.write(addr, value),
            0x8000..0x9FFF => self.vram[(a & 0x1FFF) + (self.vram_bank.us() * 0x2000)] = value,
            0xC000..0xCFFF => self.wram[(a & 0x0FFF)] = value,
            0xD000..0xDFFF => self.wram[(a & 0x0FFF) + (self.wram_bank.us() * 0x1000)] = value,
            0xE000..0xFDFF => self.wram[a & 0x1FFF] = value,
            0xFE00..0xFE9F => self.oam[a & 0xFF] = value,
            _ => self.write_high(addr & 0x00FF, value),
        }
    }

    fn write_high(&mut self, addr: u16, value: u8) {
        match addr {
            VRAM_SELECT if self.cgb => {
                self.vram_bank = value & 1;
                self[VRAM_SELECT] = value | 0xFE;
            }
            WRAM_SELECT if self.cgb => {
                self.vram_bank = u8::max(1, value & 7);
                self[VRAM_SELECT] = value | 0xF8;
            }

            IF => self[IF] = value | 0xE0,
            IE => self[IE] = value | 0xE0,
            BOOTROM_DISABLE => self.bootrom = None,

            DIV | TAC => self.timer.write(addr, value),

            _ => self[addr] = value,
        }
    }

    pub fn read16(&self, addr: u16) -> u16 {
        let low = self.read(addr);
        let high = self.read(addr + 1);
        (high.u16() << 8) | low.u16()
    }

    pub fn write16(&mut self, addr: u16, value: u16) {
        self.write(addr, value.u8());
        self.write(addr + 1, (value << 8).u8());
    }
}

impl Index<u16> for Mmu {
    type Output = u8;

    fn index(&self, index: u16) -> &Self::Output {
        &self.high[index.us()]
    }
}

impl IndexMut<u16> for Mmu {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        &mut self.high[index.us()]
    }
}

impl Default for Mmu {
    fn default() -> Self {
        Self {
            vram: [0; 16384],
            vram_bank: 0,
            wram: [0; 16384],
            wram_bank: 0,
            oam: [0; 160],
            high: [0xFF; 256],

            bootrom: Some(BOOTIX_ROM),
            cgb: false,

            cart: Cartridge::from_rom(vec![]),
            timer: Timer::default(),
        }
    }
}

impl GameGirl {
    fn timer(&mut self) -> &mut Timer {
        &mut self.mmu.timer
    }
}
