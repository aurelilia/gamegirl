// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{
    components::{
        arm::{
            interface::{ArmSystem, RwType},
            registers::Flag::IrqDisable,
            Access, Cpu, Exception,
        },
        memory::MemoryMapper,
    },
    numutil::NumExt,
};

use crate::{
    addr::{IE_H, IE_L, IF_H, IF_L, IME},
    Nds, Nds7, Nds9, NdsCpu,
};

pub const NDS9_CLOCK: u32 = 67_027_964;

impl ArmSystem for Nds7 {
    const IS_V5: bool = false;
    const IF_ADDR: u32 = IF_L;

    fn cpur(&self) -> &Cpu<Self> {
        &self.cpu7
    }

    fn cpu(&mut self) -> &mut Cpu<Self> {
        &mut self.cpu7
    }

    fn advance_clock(&mut self) {}

    fn add_sn_cycles(&mut self, cycles: u16) {
        self.time_7 += cycles.u32() << 1;
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.time_7 += cycles.u32() << 1;
    }

    fn is_irq_pending(&self) -> bool {
        is_irq_pending(self)
    }

    fn exception_happened(&mut self, _kind: Exception) {}

    fn pipeline_stalled(&mut self) {}

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        MemoryMapper::get(self, addr, T::WIDTH - 1, Self::get_slow)
    }

    fn set<T: RwType>(&mut self, addr: u32, value: T) {
        MemoryMapper::set(self, addr, value, Self::set_slow);
    }

    fn wait_time<T: RwType>(&mut self, _addr: u32, _access: Access) -> u16 {
        1
    }

    fn check_debugger(&mut self) -> bool {
        true
    }

    fn can_cache_at(_addr: u32) -> bool {
        false
    }
}

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
        self.scheduler.advance(cycles.u32());
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.scheduler.advance(cycles.u32());
    }

    fn is_irq_pending(&self) -> bool {
        is_irq_pending(self)
    }

    fn exception_happened(&mut self, _kind: Exception) {}

    fn pipeline_stalled(&mut self) {}

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        MemoryMapper::get(self, addr, T::WIDTH - 1, Self::get_slow)
    }

    fn set<T: RwType>(&mut self, addr: u32, value: T) {
        MemoryMapper::set(self, addr, value, Self::set_slow);
    }

    fn wait_time<T: RwType>(&mut self, _addr: u32, _access: Access) -> u16 {
        1
    }

    fn check_debugger(&mut self) -> bool {
        true
    }

    fn can_cache_at(_addr: u32) -> bool {
        false
    }
}

fn is_irq_pending<DS: NdsCpu>(ds: &DS) -> bool {
    (ds[IME] == 1)
        && !ds.cpur().flag(IrqDisable)
        && (((ds[IE_L] & ds[IF_L]) != 0) || ((ds[IE_H] & ds[IF_H]) != 0))
}
