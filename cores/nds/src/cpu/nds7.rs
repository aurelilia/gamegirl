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
    addr::{IE, IF, IME},
    Nds7, Nds9, NdsCpu,
};

impl ArmSystem for Nds7 {
    const IS_V5: bool = false;
    const IF_ADDR: u32 = IF;
    const EXCEPTION_VECTOR_BASE: u32 = 0;

    fn cpur(&self) -> &Cpu<Self> {
        &self.cpu7
    }

    fn cpu(&mut self) -> &mut Cpu<Self> {
        &mut self.cpu7
    }

    fn advance_clock(&mut self) {}

    fn add_sn_cycles(&mut self, cycles: u16) {
        self.time_7 += (cycles as Time) << 1;
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.time_7 += (cycles as Time) << 1;
    }

    fn exception_happened(&mut self, _kind: Exception) {}

    fn pipeline_stalled(&mut self) {}

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        Nds7::get(self, addr)
    }

    fn set<T: RwType>(&mut self, addr: u32, value: T) {
        Nds7::set(self, addr, value)
    }

    fn wait_time<T: RwType>(&mut self, _addr: u32, _access: Access) -> u16 {
        1
    }

    fn debugger(&mut self) -> &mut Debugger {
        &mut self.c.debugger
    }

    fn will_execute(&mut self, _pc: u32) {}
}
