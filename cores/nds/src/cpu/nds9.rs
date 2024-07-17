// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use arm_cpu::{
    interface::{ArmSystem, RwType},
    registers::Flag::IrqDisable,
    Access, Cpu, Exception,
};
use common::{
    common::debugger::Debugger,
    components::memory_mapper::{MemoryMappedSystem, MemoryMapper},
    numutil::NumExt,
    Time,
};

use crate::{
    addr::{IE_H, IE_L, IF_H, IF_L, IME},
    Nds7, Nds9, NdsCpu,
};

impl ArmSystem for Nds9 {
    const IS_V5: bool = true;
    const IF_ADDR: u32 = IF_L;

    fn cpur(&self) -> &Cpu<Self> {
        &self.cpu9
    }

    fn cpu(&mut self) -> &mut Cpu<Self> {
        &mut self.cpu9
    }

    fn advance_clock(&mut self) {
        if self.scheduler.has_events() {
            while let Some(event) = self.scheduler.get_next_pending() {
                event.kind.dispatch(self, event.late_by);
            }
        }
    }

    fn add_sn_cycles(&mut self, cycles: u16) {
        self.scheduler.advance(cycles as Time);
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.scheduler.advance(cycles as Time);
    }

    fn exception_happened(&mut self, _kind: Exception) {}

    fn pipeline_stalled(&mut self) {}

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        Nds9::get(self, addr)
    }

    fn set<T: RwType>(&mut self, addr: u32, value: T) {
        Nds9::set(self, addr, value)
    }

    fn wait_time<T: RwType>(&mut self, _addr: u32, _access: Access) -> u16 {
        1
    }

    fn debugger(&mut self) -> &mut Debugger {
        &mut self.c.debugger
    }

    fn can_cache_at(_addr: u32) -> bool {
        false
    }

    fn will_execute(&mut self, _pc: u32) {}

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

            (1, 0, 0) => self.cp15.control = rd.into(),

            // PU
            (2, 0, 0 | 1) => self.cp15.cache_bits[cp.us()] = rd.u8(),
            (3, 0, 0) => self.cp15.data_bufferable_bits = rd.u8(),
            (5, 0, 0 | 1) => self.cp15.access_protection_bits[cp.us()] = rd.u16(),
            (5, 0, 2 | 3) => self.cp15.access_protection_bits_ext[cp.us() - 2] = rd,
            (6, _, 0 | 1) => self.cp15.protection_unit_regions[cp.us()][cm.us()] = rd,

            (7, 0, 4) => self.cpu9.halt_on_irq(),
            // TODO Cache control stuff?
            (9, 0, 0 | 1) => self.cp15.cache_lockdown[cp.us()] = rd,
            (9, 1, 0 | 1) => self.cp15.tcm_control[cp.us()] = rd.into(),

            (13, 0 | 1, 1) => self.cp15.trace_process_id = rd,

            _ => (),
        }
    }
}
