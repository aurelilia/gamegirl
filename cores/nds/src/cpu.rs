// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! CPU implementations.
//! Note that when it comes to timing, the ARM9 runs on the scheduler until
//! the ARM7 is behind, which then runs outside the scheduler until the ARM9 is
//! behind. This is repeated in a loop.
//! Effectively, the ARM9 is the one handling the scheduling, with the ARM7
//! being dragged along.

use arm_cpu::{
    interface::{ArmSystem, RwType},
    registers::Flag::IrqDisable,
    Access, Cpu, Exception,
};
use common::{
    components::{debugger::Debugger, memory::MemoryMapper},
    numutil::NumExt,
    Time,
};

use crate::{
    addr::{IE_H, IE_L, IF_H, IF_L, IME},
    Nds7, Nds9, NdsCpu,
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
        self.time_7 += (cycles as Time) << 1;
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.time_7 += (cycles as Time) << 1;
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

    fn debugger(&mut self) -> &mut Debugger<u32> {
        &mut self.debugger
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
        self.scheduler.advance(cycles as Time);
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.scheduler.advance(cycles as Time);
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

    fn debugger(&mut self) -> &mut Debugger<u32> {
        &mut self.debugger
    }

    fn can_cache_at(_addr: u32) -> bool {
        false
    }
}
