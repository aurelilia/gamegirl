// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::vec::Vec;
use core::default;

use arm_cpu::{Cpu, Interrupt};
use common::{
    components::{io::IoSection, scheduler::Scheduler},
    numutil::{dword, word, ByteArrayExt, NumExt},
};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use crate::{
    scheduling::{CartEvent, NdsEvent},
    Nds, NdsCpu,
};

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Control {
    baud: B2,
    #[skip]
    __: B4,
    hold_chipselect: bool,
    busy: bool,
    #[skip]
    __: B5,
    slot_mode: SlotMode,
    rom_complete_irq: bool,
    slot_en: bool,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 1]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum SlotMode {
    Rom = 0,
    Serial = 1,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RomRead {
    #[default]
    EndlessFF,
    Rom(u32),
    Word(u32),
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cartridge {
    #[cfg_attr(feature = "serde", serde(skip, default))]
    pub rom: Vec<u8>,

    // SPI
    pub spictrl: Control,
    pub spidata: u16,

    // ROM
    pub romcmd: [u32; 2],
    pub romctrl: u32,
    pub rom_read_addr: RomRead,
    pub rom_read_left: u32,
}

impl Cartridge {
    pub fn load_rom(&mut self, rom: Vec<u8>) {
        self.rom = rom;
    }

    pub fn header(&self) -> CartridgeHeader {
        self.rom.get_exact(0)
    }

    pub fn handle_evt(&mut self, evt: CartEvent) -> bool {
        match evt {
            CartEvent::SpiDataComplete => {
                self.spictrl.set_busy(false);
            }

            CartEvent::RomTransferReady => {
                self.romctrl = self.romctrl.set_bit(23, true);
                self.romctrl = self.romctrl.set_bit(31, false);
                return true;
            }
        }

        false
    }

    pub fn data_write(&mut self, sched: &mut Scheduler<NdsEvent>, value: u16) {
        self.spictrl.set_busy(true);
        sched.schedule(NdsEvent::CartEvent(CartEvent::SpiDataComplete), 20); // TODO timing
        log::error!("CART:SPI Write 0x{value:X}");
    }

    pub fn cmd_write(&mut self, value: IoSection<u32>, low: bool) {
        let idx = (!low) as usize;
        value.apply(&mut self.romcmd[idx]);
    }

    pub fn romctrl_write(&mut self, sched: &mut Scheduler<NdsEvent>, value: IoSection<u32>) {
        value.mask(0xFF7F7FFF).apply(&mut self.romctrl);
        if value.raw().is_bit(31) {
            // TODO timing
            sched.schedule(NdsEvent::CartEvent(CartEvent::RomTransferReady), 20);

            let cmd = dword(self.romcmd[0], self.romcmd[1]);
            log::error!("CART:ROM Command 0x{cmd:X}");
            let left = match cmd & 0xFF {
                0x00 => {
                    self.rom_read_addr = RomRead::Rom(0);
                    0x200
                }

                0x9F => {
                    self.rom_read_addr = RomRead::EndlessFF;
                    0x2000
                }

                0x90 => {
                    // Thank you to Dust!
                    let chip_id = 0x0000_00C2
                        | match self.rom.len() as u32 {
                            0..=0xF_FFFF => 0,
                            len @ 0x10_0000..=0xFFF_FFFF => (len >> 20) - 1,
                            len @ 0x1000_0000..=0xFFFF_FFFF => 0x100 - (len >> 28),
                        };
                    self.rom_read_addr = RomRead::Word(chip_id);
                    0x4
                }

                unk => {
                    log::error!("CART:ROM Unknown Command 0x{unk:X}");
                    self.rom_read_addr = RomRead::EndlessFF;
                    0x0
                }
            };

            // Pretend there is no cart for now (TODO)
            self.rom_read_addr = RomRead::EndlessFF;
            self.rom_read_left = match self.romctrl.bits(24, 3) {
                0 => 0,
                7 => 4,
                v => 0x100 << v,
            };
            self.rom_read_left = self.rom_read_left.min(left);
        }
    }

    pub fn data_in_read(ds: &mut impl NdsCpu) -> u32 {
        let dsx: &mut Nds = &mut *ds;
        let value = match &mut dsx.cart.rom_read_addr {
            RomRead::EndlessFF => u32::MAX,
            RomRead::Word(word) => *word,
            RomRead::Rom(addr) => {
                let value = dsx.cart.rom.get_exact(*addr as usize);
                *addr += 4;
                value
            }
        };

        ds.cart.rom_read_left -= 4;
        if ds.cart.rom_read_left == 0 && ds.cart.spictrl.rom_complete_irq() {
            Cpu::request_interrupt(ds, Interrupt::CardTransferComplete)
        }
        ds.cart.romctrl = ds.cart.romctrl.set_bit(23, false);
        ds.cart.romctrl = ds.cart.romctrl.set_bit(31, true);
        ds.scheduler
            .schedule(NdsEvent::CartEvent(CartEvent::RomTransferReady), 20);

        value
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
