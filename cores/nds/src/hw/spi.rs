// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::{hword, word, NumExt, U32Ext};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use crate::{io::IoSection, CpuDevice};

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Control {
    baud: B2,
    #[skip]
    __: B5,
    busy: bool,
    dev: DevSelect,
    transfer_16bit: bool,
    chipselect_hold: bool,
    #[skip]
    __: B2,
    irq_enable: bool,
    bus_enable: bool,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DevSelect {
    PowerManagement = 0,
    Firmware = 1,
    Touchscreen = 2,
    Reserved = 3,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SpiBus {
    pub ctrl: Control,
    pub data_out: u16,

    pub(crate) firm_data: Box<[u8]>,
    firm: FirmwareState,
    firm_write_en: bool,
}

#[derive(Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum FirmwareState {
    #[default]
    AwaitingCmd,
    ReadStatusWaiting,
    ReadStatus,
    ReadWaitingAddr {
        addr: Vec<u8>,
    },
    Read {
        addr: u32,
    },
}

impl SpiBus {
    pub fn ctrl_write(&mut self, value: IoSection<u16>) {
        let prev = self.ctrl;
        value.mask(0xCF83).apply_io(&mut self.ctrl);
    }

    pub fn data_write(&mut self, value: u16) {
        match self.ctrl.dev() {
            DevSelect::PowerManagement => log::error!("PWMAN: Write 0x{value:X}"),

            DevSelect::Firmware => {
                self.data_out = 0xFF;
                match &mut self.firm {
                    FirmwareState::AwaitingCmd => match value & 0xFF {
                        0x03 => self.firm = FirmwareState::ReadWaitingAddr { addr: vec![] },
                        0x05 => self.firm = FirmwareState::ReadStatusWaiting,
                        0x06 => self.firm_write_en = true,
                        0x04 => self.firm_write_en = false,
                        _ => log::error!("FIRM: Unknown command 0x{value:X}"),
                    },

                    FirmwareState::ReadStatusWaiting => {
                        self.firm = FirmwareState::ReadStatus;
                        self.data_out = (self.firm_write_en as u16) << 1;
                    }
                    FirmwareState::ReadStatus => {
                        self.data_out = (self.firm_write_en as u16) << 1;
                    }

                    FirmwareState::ReadWaitingAddr { addr } if addr.len() < 3 => {
                        addr.push(value.u8())
                    }
                    FirmwareState::ReadWaitingAddr { addr } => {
                        let mut addr = word(hword(addr[2], addr[1]), addr[0].u16());
                        self.data_out = self.firm_data[addr.us()].u16();
                        addr += 1;
                        self.firm = FirmwareState::Read { addr }
                    }

                    FirmwareState::Read { ref mut addr } => {
                        self.data_out = self.firm_data[addr.us()].u16();
                        *addr += 1;
                    }
                }
            }

            DevSelect::Touchscreen => log::error!("TSC: Write 0x{value:X}"),
            DevSelect::Reserved => (),
        }

        if !self.ctrl.chipselect_hold() {
            match self.ctrl.dev() {
                DevSelect::Firmware => self.firm = FirmwareState::AwaitingCmd,
                _ => (),
            };
        }
    }
}
