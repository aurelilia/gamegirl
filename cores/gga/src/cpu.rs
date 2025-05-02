// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use core::{
    mem,
    ops::{Deref, DerefMut},
};

use armchair::{
    interface::{Arm7Dtmi, Bus, BusCpuConfig, RwType},
    optimizations::Optimizations,
    Access, Address, CpuState, Exception,
};
use common::{common::debugger::Debugger, Time};

use crate::{GameGirlAdv, GgaBus};

pub const CPU_CLOCK: f32 = 2u32.pow(24) as f32;

impl Bus for GgaBus {
    type Version = Arm7Dtmi;
    const CONFIG: BusCpuConfig = BusCpuConfig {
        exception_vector_base_address: Address(0),
    };

    fn tick(&mut self, cycles: Time) {
        self.scheduler.advance(cycles);
        self.step_prefetch(cycles as u16);
    }

    fn handle_events(&mut self, cpu: &mut CpuState) {
        GgaFullBus::from_parts(cpu, self).advance_clock();
    }

    fn debugger(&mut self) -> &mut Debugger {
        &mut self.c.debugger
    }

    fn exception_happened(&mut self, cpu: &mut CpuState, kind: Exception) {
        match kind {
            Exception::Irq if cpu.pc().0 > 0x100_0000 => self.memory.bios_value = 0xE25E_F004,
            Exception::Swi => self.memory.bios_value = 0xE3A0_2004,
            _ => (),
        }
    }

    fn pipeline_stalled(&mut self, cpu: &mut CpuState) {
        GgaFullBus::from_parts(cpu, self).stop_prefetch();
    }

    fn get<T: RwType>(&mut self, cpu: &mut CpuState, addr: Address) -> T {
        GgaFullBus::from_parts(cpu, self).get(addr)
    }

    fn set<T: RwType>(&mut self, cpu: &mut CpuState, addr: Address, value: T) {
        GgaFullBus::from_parts(cpu, self).set(addr, value)
    }

    fn wait_time<T: RwType>(&mut self, cpu: &mut CpuState, addr: Address, access: Access) -> u16 {
        GgaFullBus::from_parts(cpu, self).wait_time::<T>(addr, access)
    }
}

#[repr(C)]
pub struct GgaFullBus {
    pub cpu: CpuState,
    pub bus: GgaBus,
    pub opt: Optimizations<GgaBus>,
}

impl GgaFullBus {
    /// Build a full bus from it's parts.
    /// The 2 parts _must_ be from the same system, otherwise this _will_ panic!
    pub fn from_parts<'s>(cpu: &'s mut CpuState, bus: &'s mut GgaBus) -> &'s mut Self {
        let transmuted: &'s mut Self = unsafe { mem::transmute(cpu) };
        debug_assert_eq!(bus as *const _, (&transmuted.bus) as *const _);
        transmuted
    }
}

impl Deref for GgaFullBus {
    type Target = GgaBus;

    fn deref(&self) -> &Self::Target {
        &self.bus
    }
}

impl DerefMut for GgaFullBus {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bus
    }
}

impl<'g> From<&'g GameGirlAdv> for &'g GgaFullBus {
    fn from(value: &'g GameGirlAdv) -> Self {
        unsafe { mem::transmute(&value.cpu) }
    }
}

impl<'g> From<&'g mut GameGirlAdv> for &'g mut GgaFullBus {
    fn from(value: &'g mut GameGirlAdv) -> Self {
        unsafe { mem::transmute(&mut value.cpu) }
    }
}
