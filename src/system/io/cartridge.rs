use crate::numutil::NumExt;
use crate::system::io::cartridge::MBCKind::*;
use std::iter;

const CGB_FLAG: u16 = 0x0143;
const CGB_ONLY: u8 = 0xC0;
const KIND: u16 = 0x0147;
const ROM_BANKS: u16 = 0x0148;
const RAM_BANKS: u16 = 0x0149;
const BANK_COUNT_1MB: u16 = 64;

#[derive(Clone)]
pub struct Cartridge {
    pub(super) rom: Vec<u8>,
    pub rom0_bank: u16,
    pub rom1_bank: u16,

    pub(super) ram: Vec<u8>,
    pub ram_bank: u8,
    pub ram_enable: bool,

    pub kind: MBCKind,
}

impl Cartridge {
    pub fn read(&self, addr: u16) -> u8 {
        let a = addr as usize;
        match addr {
            0x0000..=0x3FFF => self.rom[a + (0x4000 * self.rom0_bank as usize)],
            0x4000..=0x7FFF => self.rom[(a & 0x3FFF) + (0x4000 * self.rom1_bank as usize)],
            0xA000..=0xBFFF if !self.ram.is_empty() && self.ram_enable => {
                if let MBC2 = self.kind {
                    self.ram[0x1FF]
                } else {
                    self.ram[(a & 0x1FFF) + (0x2000 * self.ram_bank.us())]
                }
            }
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match (&mut self.kind, addr) {
            // MBC2
            (MBC2, 0x0000..=0x3FFF) if addr.is_bit(8) => {
                self.rom1_bank = (value.u16() & 0x0F).max(1) % self.rom_bank_count()
            }
            (MBC2, 0x0000..=0x3FFF) => self.ram_enable = (value & 0x0F) == 0x0A,
            (MBC2, 0xA000..=0xBFFF) if self.ram_enable => {
                self.ram[addr.us() & 0x1FF] = value | 0xF0
            }

            // Shared between all (except MBC2...)
            (_, 0x0000..=0x1FFF) => self.ram_enable = (value & 0x0F) == 0x0A,
            (_, 0xA000..=0xBFFF) if !self.ram.is_empty() => {
                self.ram[(addr & 0x1FFF).us() + (0x2000 * self.ram_bank.us())] = value
            }

            // Shared between some
            (MBC3 | MBC5, 0x4000..=0x5FFF) => {
                self.ram_bank = (value & 0x03) % self.ram_bank_count()
            }

            // MBC1
            (MBC1 { ram_mode, bank2 }, 0x2000..=0x3FFF) => {
                self.rom1_bank = (value & 0x1F).max(1).u16();
                let (bank2, ram_mode) = (*bank2, *ram_mode);
                self.mbc1_bank2_update(bank2, ram_mode);
            }
            (MBC1 { ram_mode, bank2 }, 0x4000..=0x5FFF) => {
                *bank2 = value & 0x03;
                let (bank2, ram_mode) = (*bank2, *ram_mode);
                self.mbc1_bank2_update(bank2, ram_mode);
            }
            (MBC1 { ram_mode, bank2 }, 0x6000..=0x7FFF) => {
                *ram_mode = value.is_bit(0);
                let (bank2, ram_mode) = (*bank2, *ram_mode);
                self.mbc1_bank2_update(bank2, ram_mode);
            }

            // MBC3
            (MBC3, 0x2000..=0x3FFF) => {
                self.rom1_bank = value.max(1).u16() % self.rom_bank_count();
            }

            // MBC5
            (MBC5, 0x2000..=0x2FFF) => {
                self.rom1_bank = (self.rom1_bank & 0x100) | (value.u16() % self.rom_bank_count())
            }
            (MBC5, 0x3000..=0x3FFF) => {
                self.rom1_bank = self.rom1_bank.set_bit(8, value.is_bit(0)) % self.rom_bank_count()
            }

            _ => (),
        }
    }

    fn mbc1_bank2_update(&mut self, bank2: u8, ram_mode: bool) {
        self.ram_bank = if self.ram_bank_count() == 4 && ram_mode {
            bank2
        } else {
            0
        };
        self.rom1_bank &= self.rom1_bank;
        if self.rom_bank_count() >= BANK_COUNT_1MB {
            self.rom1_bank += bank2.u16() << 5;
        }
        self.rom1_bank %= self.rom_bank_count();
        self.rom0_bank = if ram_mode && self.rom_bank_count() >= BANK_COUNT_1MB {
            (bank2.u16() << 5) % self.rom_bank_count()
        } else {
            0
        };
    }

    pub fn rom_bank_count(&self) -> u16 {
        2 << self.rom[ROM_BANKS.us()].u16()
    }

    pub fn ram_bank_count(&self) -> u8 {
        match self.rom[RAM_BANKS.us()] {
            0 if self.kind == MBC2 => 1,
            0 => 0,
            2 => 1,
            3 => 4,
            4 => 16,
            5 => 8,
            _ => panic!("Unknown cartridge controller"),
        }
    }

    pub fn supports_cgb(&self) -> bool {
        self.rom[CGB_FLAG.us()].is_bit(7)
    }

    pub fn requires_cgb(&self) -> bool {
        self.rom[CGB_FLAG.us()] == CGB_ONLY
    }

    pub fn title(&self, extended: bool) -> String {
        let mut buf = String::with_capacity(20);
        let end = if extended { 0x0142 } else { 0x013E };
        for b in 0x134..=end {
            let value = self.rom[b];
            if value == 0 {
                break;
            }
            buf.push(value as char);
        }
        buf
    }

    pub fn from_rom(rom: Vec<u8>) -> Self {
        let kind = rom[KIND as usize];
        let mut cart = Self {
            rom,
            kind: match kind {
                0x01..0x04 => MBC1 {
                    ram_mode: false,
                    bank2: 0,
                },
                0x05..=0x06 => MBC2,
                0x0F..=0x10 => MBC3, // TODO RTC variant
                0x11..=0x13 => MBC3,
                0x19..=0x1E => MBC5,
                _ => NoMBC,
            },
            ..Self::dummy()
        };
        cart.ram
            .extend(iter::repeat(0).take(0x2000 * cart.ram_bank_count().us()));
        cart
    }

    pub fn dummy() -> Self {
        Self {
            rom: vec![],
            rom0_bank: 0,
            rom1_bank: 1,
            ram: vec![],
            ram_bank: 0,
            ram_enable: false,
            kind: NoMBC,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MBCKind {
    NoMBC,
    MBC1 { ram_mode: bool, bank2: u8 },
    MBC2,
    MBC3,
    MBC5,
}
