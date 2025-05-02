// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use armchair::{
    interface::{Arm7Dtmi, Bus, BusCpuConfig, RwType},
    state::Flag::IrqDisable,
    Access, Address, Cpu, CpuState, Exception,
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

impl Bus for Nds7 {
    type Version = Arm7Dtmi;

    const CONFIG: BusCpuConfig = BusCpuConfig {
        exception_vector_base_address: Address(0),
    };

    fn tick(&mut self, cycles: Time) {
        self.time_7 += cycles << 1;
    }

    fn handle_events(&mut self, cpu: &mut CpuState) {
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
}
