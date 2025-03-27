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
use core::ops::Range;

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

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TcmState {
    None,
    Wo,
    Rw,
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

    pub(crate) tcm_state: [TcmState; 2],
    pub(crate) tcm_range: [Range<u32>; 2],
}

impl Cp15 {
    pub fn dtcm_map_update(&mut self) {
        let base = self.tcm_control[0].region_base() << 12;
        let size = 512 << self.tcm_control[0].virtual_size();
        self.tcm_range[0] = base..(base + size);
        self.tcm_state[0] = if self.control.dtcm_enable() {
            if self.control.dtcm_load_mode() {
                TcmState::Wo
            } else {
                TcmState::Rw
            }
        } else {
            TcmState::None
        };
    }
    pub fn itcm_map_update(&mut self) {
        let size = 512 << self.tcm_control[1].virtual_size();
        self.tcm_range[1] = 0..size;
        self.tcm_state[1] = if self.control.itcm_enable() && size > 0x200_0000 {
            if self.control.itcm_load_mode() {
                TcmState::Wo
            } else {
                TcmState::Rw
            }
        } else {
            TcmState::None
        };
    }

    pub fn control_update(&mut self, ctrl: u32) {
        self.control = ctrl.into();
        self.dtcm_map_update();
        self.itcm_map_update();
    }
}

impl Default for Cp15 {
    fn default() -> Self {
        Self {
            control: Control::default()
                .with_dtcm_enable(true)
                .with_itcm_enable(true),
            cache_bits: [0; 2],
            data_bufferable_bits: 0,
            access_protection_bits: [0; 2],
            access_protection_bits_ext: [0; 2],
            protection_unit_regions: [[0; 8]; 2],
            cache_lockdown: [0; 2],
            tcm_control: [
                TcmControl::default()
                    .with_region_base(0x27C0)
                    .with_virtual_size(5),
                TcmControl::default().with_virtual_size(12),
            ],
            trace_process_id: 0,

            tcm_state: [TcmState::Rw, TcmState::None],
            tcm_range: [0x27C_0000..0x280_0000, 0x000_0000..0x200_0000],
        }
    }
}
