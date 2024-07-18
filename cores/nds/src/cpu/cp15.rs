// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! Inner implementation of CP15 for the ARMv5.
//! Note that getters and setters are in `nds9.rs`, as part of the ARM
//! interface.
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

#[bitfield]
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Control {
    pub mmu_enable: bool,
    #[skip]
    __: B1,
    pub data_cache_enable: bool,
    #[skip]
    __: B4,
    pub big_endian: bool,
    #[skip]
    __: B4,

    pub inst_cache_enable: bool,
    pub exception_vectors_high: bool,
    pub cache_replacement: bool,
    pub pre_armv5: bool,

    pub dtcm_enable: bool,
    pub dtcm_load_mode: bool,
    pub itcm_enable: bool,
    pub itcm_load_mode: bool,

    #[skip]
    __: B12,
}

#[bitfield]
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TcmControl {
    #[skip]
    __: B1,
    pub virtual_size: B5,
    #[skip]
    __: B6,
    pub region_base: B20,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cp15 {
    pub(crate) control: Control,
    pub(crate) cache_bits: [u8; 2],
    pub(crate) data_bufferable_bits: u8,

    pub(crate) access_protection_bits: [u16; 2],
    pub(crate) access_protection_bits_ext: [u32; 2],
    pub(crate) protection_unit_regions: [[u32; 8]; 2],

    pub(crate) cache_lockdown: [u32; 2],
    pub(crate) tcm_control: [TcmControl; 2],
    pub(crate) trace_process_id: u32,
}

impl Cp15 {
    pub fn dtcm_region(&self) -> u32 {
        self.tcm_control[1].region_base() >> 12
    }
}

impl Default for Cp15 {
    fn default() -> Self {
        Self {
            control: Control::default(),
            cache_bits: [0; 2],
            data_bufferable_bits: 0,
            access_protection_bits: [0; 2],
            access_protection_bits_ext: [0; 2],
            protection_unit_regions: [[0; 8]; 2],
            cache_lockdown: [0; 2],
            tcm_control: [
                TcmControl::default(),
                TcmControl::default().with_region_base(0x27C0),
            ],
            trace_process_id: 0,
        }
    }
}
