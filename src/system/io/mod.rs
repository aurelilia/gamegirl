use crate::numutil::NumExt;
use crate::system::io::addr::*;
use crate::system::io::apu::Apu;
use crate::system::io::cartridge::Cartridge;
use crate::system::io::dma::{Dma, Hdma};
use crate::system::io::joypad::Joypad;
use crate::system::io::ppu::Ppu;
use crate::system::io::timer::Timer;
use crate::system::GameGirl;
use std::{
    ops::{Index, IndexMut},
    sync::{Arc, RwLock},
};

use super::debugger::Debugger;

pub(super) mod addr;
pub mod apu;
pub(super) mod cartridge;
mod dma;
pub mod joypad;
mod ppu;
mod timer;

pub struct Mmu {
    vram: [u8; 2 * 8192],
    vram_bank: u8,
    wram: [u8; 4 * 8192],
    wram_bank: u8,
    oam: [u8; 160],
    high: [u8; 256],

    bootrom: Option<&'static [u8]>,
    cgb: bool,
    debugger: Option<Arc<RwLock<Debugger>>>,

    cart: Cartridge,
    timer: Timer,
    pub ppu: Ppu,
    joypad: Joypad,
    dma: Dma,
    pub(super) apu: Apu,
    hdma: Hdma,
}

impl Mmu {
    pub(super) fn step(gg: &mut GameGirl, m_cycles: usize, t_cycles: usize) {
        let t_cpu = m_cycles * 4;
        Hdma::step(gg);
        Timer::step(gg, t_cpu);
        Ppu::step(gg, t_cycles);
        Dma::step(gg, t_cpu);
        Apu::step(&mut gg.mmu, t_cycles);
    }

    pub fn read(&self, addr: u16) -> u8 {
        let a = addr.us();
        match addr {
            0x0000..=0x0100 if self.bootrom.is_some() => self.bootrom.unwrap()[a],
            0x0200..=0x0900 if self.bootrom.is_some() && self.cgb => {
                self.bootrom.unwrap()[a - 0x0100]
            }
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.cart.read(addr),

            0x8000..=0x9FFF => self.vram[(a & 0x1FFF) + (self.vram_bank.us() * 0x2000)],
            0xC000..=0xCFFF => self.wram[(a & 0x0FFF)],
            0xD000..=0xDFFF => self.wram[(a & 0x0FFF) + (self.wram_bank.us() * 0x1000)],
            0xE000..=0xFDFF => self.wram[a & 0x1FFF],
            0xFE00..=0xFE9F => self.oam[a & 0xFF],

            _ => self.read_high(addr & 0x00FF),
        }
    }

    pub(super) fn read_signed(&self, addr: u16) -> i8 {
        self.read(addr) as i8
    }

    fn read_high(&self, addr: u16) -> u8 {
        match addr {
            JOYP => self.joypad.read(self.high[JOYP as usize]),
            DIV | TAC => self.timer.read(addr),

            LY if !self[LCDC].is_bit(7) => 0,
            BCPS..=OCPD => self.ppu.read_high(addr),

            HDMA_START if self.cgb => self.hdma.transfer_left as u8,
            HDMA_SRC_HIGH..=HDMA_DEST_LOW => 0xFF,

            _ => self[addr],
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        let a = addr.us();
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.cart.write(addr, value),
            0x8000..=0x9FFF => self.vram[(a & 0x1FFF) + (self.vram_bank.us() * 0x2000)] = value,
            0xC000..=0xCFFF => self.wram[(a & 0x0FFF)] = value,
            0xD000..=0xDFFF => self.wram[(a & 0x0FFF) + (self.wram_bank.us() * 0x1000)] = value,
            0xFE00..=0xFE9F => self.oam[a & 0xFF] = value,
            0xFF00..=0xFFFF => self.write_high(addr & 0x00FF, value),
            _ => (),
        }
    }

    fn write_high(&mut self, addr: u16, value: u8) {
        match addr {
            VRAM_SELECT if self.cgb => {
                self.vram_bank = value & 1;
                self[VRAM_SELECT] = value | 0xFE;
            }
            WRAM_SELECT if self.cgb => {
                self.wram_bank = u8::max(1, value & 7);
                self[WRAM_SELECT] = value | 0xF8;
            }

            IF => self[IF] = value | 0xE0,
            IE => self[IE] = value | 0xE0,
            BOOTROM_DISABLE => self.bootrom = None,

            DIV | TAC => self.timer.write(addr, value),
            LCDC => {
                self[LCDC] = value;
                if !value.is_bit(7) {
                    self[STAT] &= 0xF8;
                }
            }
            DMA => {
                self[addr] = value;
                self.dma.start();
            }
            BCPS..=OCPD => self.ppu.write_high(addr, value),
            HDMA_START => Hdma::write_start(self, value),
            KEY1 => self[KEY1] = (value & 1) | self[KEY1] & 0x80,

            0x01 if self.debugger.is_some() => self
                .debugger
                .as_ref()
                .unwrap()
                .write()
                .unwrap()
                .serial_output
                .push(value as char),

            LY | SC => (),
            _ => self[addr] = value,
        }
    }

    pub fn read16(&self, addr: u16) -> u16 {
        let low = self.read(addr);
        let high = self.read(addr.wrapping_add(1));
        (high.u16() << 8) | low.u16()
    }

    pub fn write16(&mut self, addr: u16, value: u16) {
        self.write(addr, value.u8());
        self.write(addr.wrapping_add(1), (value >> 8).u8());
    }

    pub(super) fn reset(&mut self) -> Self {
        // TODO the clones are kinda eh
        let mut new = Self::new(self.debugger.clone());
        new.load_cart(self.cart.clone());
        new
    }

    pub(super) fn new(debugger: Option<Arc<RwLock<Debugger>>>) -> Self {
        let mut mmu = Self {
            vram: [0; 16384],
            vram_bank: 0,
            wram: [0; 32768],
            wram_bank: 1,
            oam: [0; 160],
            high: [0xFF; 256],

            bootrom: None,
            cgb: false,
            debugger,

            timer: Timer::default(),
            ppu: Ppu::new(),
            joypad: Joypad::default(),
            dma: Dma::default(),
            apu: Apu::default(),
            hdma: Hdma::default(),
            cart: Cartridge::dummy(),
        };
        mmu.init_high();
        mmu
    }

    pub fn load_cart(&mut self, cart: Cartridge) {
        self.bootrom = Some(if cart.supports_cgb() {
            CGB_BOOTROM
        } else {
            BOOTIX_ROM
        });
        self.cgb = cart.supports_cgb();
        self.ppu.switch_kind(self.cgb);
        self.cart = cart;
    }

    fn init_high(&mut self) {
        self[LY] = 0;
        self[LCDC] = 0;
        self[STAT] = 0;
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
        self[KEY1] = 0;
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
