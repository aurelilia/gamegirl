use crate::numutil::NumExt;

const CGB_FLAG: u16 = 0x0143;
const CGB_ONLY: u8 = 0xC0;
const DMG_AND_CGB: u8 = 0x80;
const KIND: u16 = 0x0147;
const ROM_BANKS: u16 = 0x0148;
const RAM_BANKS: u16 = 0x0149;
const DESTINATION: u16 = 0x014A;
const BANK_COUNT_1MB: u8 = 64;

pub struct Cartridge {
    pub rom: Vec<u8>,
    pub rom_bank: u16,
    pub ram: Vec<u8>,
    pub ram_bank: Option<u8>,
    pub kind: MBCKind,
}

impl Cartridge {
    pub fn read(&self, addr: u16) -> u8 {
        let a = addr as usize;
        match addr {
            0x0000..0x3FFF => self.rom[a],
            0x4000..0x7FFF => self.rom[(a & 0x3FFF) + (0x4000 * self.rom_bank as usize)],
            0xA000..0xBFFF if self.ram_bank.is_some() => {
                self.ram[(a & 0x1FFF) + (0x2000 * (self.ram_bank.unwrap() - 1).us())]
            }
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        // TODO
    }

    fn rom_bank_count(&self) -> u16 {
        2 << self.rom[ROM_BANKS.us()].u16()
    }

    fn ram_bank_count(&self) -> u8 {
        match self.rom[RAM_BANKS.us()] {
            0 => 0,
            2 => 1,
            3 => 4,
            4 => 16,
            5 => 8,
            _ => panic!("Unknown cartridge controller"),
        }
    }

    fn supports_cgb(&self) -> bool {
        self.rom[CGB_FLAG.us()].is_bit(7)
    }

    fn requires_cgb(&self) -> bool {
        self.rom[CGB_FLAG.us()] == CGB_ONLY
    }

    pub fn from_rom(rom: Vec<u8>) -> Self {
        Self {
            rom,
            rom_bank: 1,
            ram: vec![],
            ram_bank: None,
            kind: MBCKind::NoMBC,
        }
    }
}

pub enum MBCKind {
    NoMBC,
}
