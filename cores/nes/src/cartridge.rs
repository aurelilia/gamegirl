// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::iter;

use common::numutil::NumExt;
use modular_bitfield::{
    bitfield,
    specifiers::{B2, B4, B7},
};

use crate::Nes;

#[bitfield]
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct INesHeader {
    mirror_is_vertical: bool,
    has_battery: bool,
    has_trainer: bool,
    ignore_mirroring: bool,
    mapper_lower: B4,
    vs_unisystem: bool,
    playchoice: bool,
    ines_2_hint: B2,
    mapper_higher: B4,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Nes2Header {
    mapper_higherer: B4,
    submapper: B4,
    prg_rom_size_msb: B4,
    chr_rom_size_msb: B4,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cartridge {
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr_rom: Vec<u8>,
    mapper: Mapper,
}

impl Cartridge {
    pub fn read(nes: &Nes, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => nes.cart.prg_ram[addr.us() - 0x6000],
            0x8000..=0xFFFF => {
                nes.cart.prg_rom[(addr.us() - 0x8000) & (nes.cart.prg_rom.len() - 1)]
            }
            _ => panic!(),
        }
    }

    pub fn write(_: &Nes, _: u16, _value: u8) {}

    pub fn from_rom(rom: Vec<u8>) -> Self {
        let prg_size = rom[4].us() * 16_384;
        let chr_size = rom[5].us() * 8_192;
        let header = INesHeader::from_bytes(rom[6..8].try_into().unwrap());

        let addr = 16 + (header.has_trainer() as usize * 512);
        let prg_rom = rom[addr..(addr + prg_size)].to_vec();
        let addr = addr + prg_size;
        let chr_rom = rom[addr..(addr + chr_size)].to_vec();

        let mapper = header.mapper_lower() | (header.mapper_higher() << 4);
        let mapper = match mapper {
            0 => Mapper::Nrom,
            _ => panic!("Unknown mapper!"),
        };

        let mut cart = Self {
            prg_rom,
            prg_ram: Vec::new(),
            chr_rom,
            mapper,
        };
        cart.prg_ram.extend(iter::repeat(0).take(0x2000));
        cart
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Mapper {
    #[default]
    Nrom,
}
