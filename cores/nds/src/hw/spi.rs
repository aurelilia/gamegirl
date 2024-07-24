// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::U32Ext;
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
}

impl SpiBus {
    pub fn data_write(&mut self, value: u16) {
        match self.ctrl.dev() {
            DevSelect::PowerManagement => log::error!("PWMAN: Write 0x{value:X}"),
            DevSelect::Firmware => log::error!("FIRM: Write 0x{value:X}"),
            DevSelect::Touchscreen => log::error!("TSC: Write 0x{value:X}"),
            DevSelect::Reserved => (),
        }
    }
}
