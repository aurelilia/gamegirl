// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use core::ops::Add;

use armchair::{
    interface::{Arm946Es, Bus, BusCpuConfig, RwType},
    state::Flag::IrqDisable,
    Access, Address, Cpu, CpuState, Exception,
};
use common::{
    common::debugger::Debugger,
    components::{
        memory_mapper::{MemoryMappedSystem, MemoryMapper},
        thin_pager::RW,
    },
    numutil::NumExt,
    Time,
};

use crate::{
    addr::{IE, IF, IME},
    Nds, Nds7, Nds9, NdsCpu,
};

impl Bus for Nds9 {
    type Version = Arm946Es;

    const CONFIG: BusCpuConfig = BusCpuConfig {
        exception_vector_base_address: Address(0xFFFF_0000),
    };

    fn tick(&mut self, cycles: Time) {
        self.scheduler.advance(cycles);
    }

    fn handle_events(&mut self, cpu: &mut armchair::CpuState) {
        if self.scheduler.has_events() {
            while let Some(event) = self.scheduler.get_next_pending() {
                event.kind.dispatch(self, event.late_by);
            }
        }
    }

    fn exception_happened(&mut self, _cpu: &mut CpuState, _kind: Exception) {}

    fn pipeline_stalled(&mut self, _cpu: &mut CpuState) {}

    fn get<T: RwType>(&mut self, _cpu: &mut CpuState, addr: Address) -> T {
        self.get(addr.0)
    }

    fn set<T: RwType>(&mut self, _cpu: &mut CpuState, addr: Address, value: T) {
        self.set(addr.0, value)
    }

    fn wait_time<T: RwType>(
        &mut self,
        _cpu: &mut CpuState,
        _addr: Address,
        _access: Access,
    ) -> u16 {
        1
    }

    fn debugger(&mut self) -> &mut Debugger {
        &mut self.c.debugger
    }

    fn get_cp15(&self, cm: u32, cp: u32, cn: u32) -> u32 {
        match (cn, cm, cp) {
            // ID registers
            (0, 0, 0 | 3..=7) => 0x4105_9461,
            (0, 0, 1) => 0x0F0D_2112,
            (0, 0, 2) => 0x0014_0180,

            (1, 0, 0) => self.cp15.control.into(),

            // PU
            (2, 0, 0 | 1) => self.cp15.cache_bits[cp.us()].u32(),
            (3, 0, 0) => self.cp15.data_bufferable_bits.u32(),
            (5, 0, 0 | 1) => self.cp15.access_protection_bits[cp.us()].u32(),
            (5, 0, 2 | 3) => self.cp15.access_protection_bits_ext[cp.us() - 2],
            (6, _, 0 | 1) => self.cp15.protection_unit_regions[cp.us()][cm.us()],

            // Cache
            (9, 0, 0 | 1) => self.cp15.cache_lockdown[cp.us()],
            (9, 1, 0 | 1) => self.cp15.tcm_control[cp.us()].into(),

            (13, 0 | 1, 1) => self.cp15.trace_process_id,

            _ => 0,
        }
    }

    fn set_cp15(&mut self, cm: u32, cp: u32, cn: u32, rd: u32) {
        match (cn, cm, cp) {
            (0, 0, _) => (),

            (1, 0, 0) => self.cp15.control_update(rd),

            // PU
            (2, 0, 0 | 1) => self.cp15.cache_bits[cp.us()] = rd.u8(),
            (3, 0, 0) => self.cp15.data_bufferable_bits = rd.u8(),
            (5, 0, 0 | 1) => self.cp15.access_protection_bits[cp.us()] = rd.u16(),
            (5, 0, 2 | 3) => self.cp15.access_protection_bits_ext[cp.us() - 2] = rd,
            (6, _, 0 | 1) => self.cp15.protection_unit_regions[cp.us()][cm.us()] = rd,

            (7, 0, 4) => self.cpu9.state.halt_on_irq(),
            // TODO Cache control stuff?
            (9, 0, 0 | 1) => self.cp15.cache_lockdown[cp.us()] = rd,
            (9, 1, 0) => {
                let dsx: &mut Nds = &mut *self;
                dsx.cp15.tcm_control[0] = rd.into();
                dsx.cp15.dtcm_map_update();
            }
            (9, 1, 1) => {
                self.cp15.tcm_control[1] = rd.into();
                self.cp15.itcm_map_update();
            }

            (13, 0 | 1, 1) => self.cp15.trace_process_id = rd,

            _ => (),
        }
    }
}
