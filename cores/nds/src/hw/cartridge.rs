// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::ByteArrayExt;

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cartridge {
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub rom: Vec<u8>,
}

impl Cartridge {
    pub fn load_rom(&mut self, rom: Vec<u8>) {
        self.rom = rom;
    }

    pub fn header(&self) -> CartridgeHeader {
        self.rom.get_exact(0)
    }
}

#[derive(Debug, Default)]
#[repr(packed)]
pub struct CartridgeHeader {
    pub game_title: [u8; 12],
    pub game_code: [u8; 4],
    pub maker_code: [u8; 2],
    pub unit_code: u8,
    pub encryption_seed_select: u8,
    pub chip_size: u8,
    __0: [u8; 8],
    pub region: u8,
    pub version: u8,
    pub autostart: u8,

    pub arm9_offset: u32,
    pub arm9_entry_addr: u32,
    pub arm9_ram_addr: u32,
    pub arm9_size: u32,

    pub arm7_offset: u32,
    pub arm7_entry_addr: u32,
    pub arm7_ram_addr: u32,
    pub arm7_size: u32,

    fnt_offset: u32,
    fnt_size: u32,
    fat_offset: u32,
    fat_size: u32,
    arm9_overlay_offset: u32,
    arm9_overlay_size: u32,
    arm7_overlay_offset: u32,
    arm7_overlay_size: u32,

    port_settings: [u32; 2],
    icon_offset: u32,
    secure_area_crc16: u16,
    secure_area_delay: u16,
    arm_autoload: [u32; 2],
    secure_area_disable: u64,
    total_size: u32,
    rom_header_size: u32,
    __1: u32,
    __2: u64,
    nand_rom_end: u16,
    nand_start_rw: u16,
}
