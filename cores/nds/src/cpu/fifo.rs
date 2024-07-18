// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::{hword, word, NumExt, U16Ext, U32Ext};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CpuFifo {
    pub sync: Sync,
    pub cnt: Control,
    pub buffer: [u32; 16],
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Sync {
    data_in: B4,
    #[skip]
    __: B4,
    data_out: B4,
    #[skip]
    __: B1,
    send_irq: bool,
    irq_en: bool,
    #[skip]
    __: B1,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Control {
    send_fifo: Fifo,
    send_fifo_clear: bool,
    #[skip]
    __: B4,
    recv_fifo: Fifo,
    #[skip]
    __: B3,
    error_full: bool,
    enable: bool,
}

#[bitfield(filled = false)]
#[derive(BitfieldSpecifier, Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Fifo {
    state: FifoStatus,
    irq_en: bool,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum FifoStatus {
    Regular = 0,
    Empty = 1,
    Full = 2,
}
