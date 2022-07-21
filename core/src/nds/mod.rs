// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod memory;
mod scheduling;

use std::ops::{Deref, DerefMut, Index, IndexMut};

use serde::{Deserialize, Serialize};

use crate::{
    common::{EmulateOptions, SystemConfig},
    components::{
        arm::{
            inst_arm::ArmLut,
            inst_thumb::ThumbLut,
            interface::{ArmSystem, RwType, SysWrapper},
            Access, Cpu, Exception,
        },
        debugger::Debugger,
        memory::MemoryMapper,
        scheduler::Scheduler,
    },
    gga::graphics::threading::GgaPpu,
    nds::{memory::Memory, scheduling::NdsEvent},
    numutil::NumExt,
};

macro_rules! deref {
    ($name:ident) => {
        impl Deref for $name {
            type Target = Nds;

            #[inline]
            fn deref(&self) -> &Self::Target {
                unsafe { &*self.0 }
            }
        }

        impl DerefMut for $name {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *self.0 }
            }
        }

        impl Index<u32> for $name {
            type Output = u16;

            fn index(&self, addr: u32) -> &Self::Output {
                assert!(addr < 0x3FF);
                assert_eq!(addr & 1, 0);
                &self.memory.mmio[(addr >> 1).us()]
            }
        }

        impl IndexMut<u32> for $name {
            fn index_mut(&mut self, addr: u32) -> &mut Self::Output {
                assert!(addr < 0x3FF);
                assert_eq!(addr & 1, 0);
                &mut self.memory.mmio[(addr >> 1).us()]
            }
        }

        // Satisfy serde...
        impl Default for $name {
            fn default() -> $name {
                unreachable!()
            }
        }
    };
}

deref!(Nds7);
deref!(Nds9);

#[derive(Deserialize, Serialize)]
pub struct Nds {
    cpu7: Cpu<Nds7>,
    cpu9: Cpu<Nds9>,
    pub ppus: [GgaPpu; 2],
    memory: Memory,
    scheduler: Scheduler<NdsEvent>,

    #[serde(skip)]
    #[serde(default)]
    pub debugger: Debugger<u32>,
    pub options: EmulateOptions,
    pub config: SystemConfig,
    ticking: bool,
}

impl Nds {
    fn advance_clock(&mut self) {}

    /// Add S/N cycles, which advance the system besides the CPU.
    #[inline]
    fn add_sn_cycles(&mut self, count: u16) {
        self.scheduler.advance(count.u32());
    }

    /// Add I cycles, which advance the system besides the CPU.
    #[inline]
    fn add_i_cycles(&mut self, count: u16) {
        self.scheduler.advance(count.u32());
    }

    #[inline]
    fn nds7(&mut self) -> Nds7 {
        Nds7(self as *mut Nds)
    }

    #[inline]
    fn nds9(&mut self) -> Nds9 {
        Nds9(self as *mut Nds)
    }
}

#[repr(transparent)]
struct Nds7(*mut Nds);
#[repr(transparent)]
struct Nds9(*mut Nds);

impl ArmSystem for Nds7 {
    const ARM_LUT: ArmLut<Self> = SysWrapper::<Self>::make_armv4_lut();
    const THUMB_LUT: ThumbLut<Self> = SysWrapper::<Self>::make_thumbv4_lut();
    const IE_ADDR: u32 = 0;
    const IF_ADDR: u32 = 0;
    const IME_ADDR: u32 = 0;

    fn cpur(&self) -> &Cpu<Self> {
        &self.cpu7
    }

    fn cpu(&mut self) -> &mut Cpu<Self> {
        &mut self.cpu7
    }

    fn advance_clock(&mut self) {
        Nds::advance_clock(self);
    }

    fn add_sn_cycles(&mut self, cycles: u16) {
        Nds::add_sn_cycles(self, cycles);
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        Nds::add_sn_cycles(self, cycles);
    }

    fn exception_happened(&mut self, _kind: Exception) {}

    fn pipeline_stalled(&mut self) {}

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        MemoryMapper::get(self, addr, T::WIDTH - 1, |_, _| T::from_u8(0))
    }

    fn set<T: RwType>(&mut self, addr: u32, value: T) {
        MemoryMapper::set(self, addr, value, |_, _, _| ());
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
    const ARM_LUT: ArmLut<Self> = SysWrapper::<Self>::make_armv4_lut();
    const THUMB_LUT: ThumbLut<Self> = SysWrapper::<Self>::make_thumbv4_lut();
    const IE_ADDR: u32 = 0;
    const IF_ADDR: u32 = 0;
    const IME_ADDR: u32 = 0;

    fn cpur(&self) -> &Cpu<Self> {
        &self.cpu9
    }

    fn cpu(&mut self) -> &mut Cpu<Self> {
        &mut self.cpu9
    }

    fn advance_clock(&mut self) {
        Nds::advance_clock(self);
    }

    fn add_sn_cycles(&mut self, cycles: u16) {
        Nds::add_sn_cycles(self, cycles);
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        Nds::add_i_cycles(self, cycles);
    }

    fn exception_happened(&mut self, _kind: Exception) {}

    fn pipeline_stalled(&mut self) {}

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        MemoryMapper::get(self, addr, T::WIDTH - 1, |_, _| T::from_u8(0))
    }

    fn set<T: RwType>(&mut self, addr: u32, value: T) {
        MemoryMapper::set(self, addr, value, |_, _, _| ());
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
