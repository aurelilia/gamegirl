#![feature(mixed_integer_ops)]
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod addr;
mod audio;
mod cartridge;
mod cpu;
mod dma;
mod graphics;
mod memory;
mod scheduling;
mod timer;

use std::{
    mem,
    ops::{Deref, DerefMut, Index, IndexMut},
};

use common::{
    common_functions,
    components::{
        arm::{interface::ArmSystem, Cpu},
        debugger::Debugger,
        scheduler::Scheduler,
    },
    misc::{EmulateOptions, SystemConfig},
    numutil::NumExt,
    Colour,
};
use serde::{Deserialize, Serialize};

use crate::{
    audio::Apu,
    cartridge::Cartridge,
    cpu::NDS9_CLOCK,
    dma::Dmas,
    graphics::NdsEngines,
    memory::Memory,
    scheduling::{ApuEvent, NdsEvent},
    timer::Timers,
};

macro_rules! deref {
    ($name:ident, $mmio:ident, $idx:expr) => {
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
                assert_eq!(addr & 1, 0);
                &self.memory.$mmio[(addr >> 1).us()]
            }
        }

        impl IndexMut<u32> for $name {
            fn index_mut(&mut self, addr: u32) -> &mut Self::Output {
                assert_eq!(addr & 1, 0);
                &mut self.memory.$mmio[(addr >> 1).us()]
            }
        }

        impl NdsCpu for $name {
            const I: usize = $idx;
        }

        // Satisfy serde...
        impl Default for $name {
            fn default() -> $name {
                unreachable!()
            }
        }
    };
}

deref!(Nds7, mmio7, 0);
deref!(Nds9, mmio9, 1);

#[derive(Deserialize, Serialize)]
pub struct Nds {
    cpu7: Cpu<Nds7>,
    cpu9: Cpu<Nds9>,
    pub ppu: NdsEngines,
    apu: Apu,
    memory: Memory,
    pub cart: Cartridge,
    dmas: CpuDevice<Dmas>,
    timers: CpuDevice<Timers>,

    scheduler: Scheduler<NdsEvent>,
    time_7: u32,

    #[serde(skip)]
    #[serde(default)]
    pub debugger: Debugger<u32>,
    pub options: EmulateOptions,
    pub config: SystemConfig,
    ticking: bool,
}

impl Nds {
    common_functions!(NDS9_CLOCK, NdsEvent::PauseEmulation);

    /// Step forward the emulated console including all subsystems.
    pub fn advance(&mut self) {
        // Run an instruction on the ARM9, then keep running the ARM7
        // until it has caught up
        Cpu::continue_running(&mut self.nds9());
        let mut nds7 = self.nds7();
        while self.time_7 < self.scheduler.now() {
            Cpu::continue_running(&mut nds7);
        }
    }

    fn advance_clock(&mut self) {
        if self.scheduler.has_events() {
            while let Some(event) = self.scheduler.get_next_pending() {
                event.kind.dispatch(self, event.late_by);
            }
        }
    }

    #[inline]
    fn nds7(&mut self) -> Nds7 {
        Nds7(self as *mut Nds)
    }

    #[inline]
    fn nds9(&mut self) -> Nds9 {
        Nds9(self as *mut Nds)
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
        self.init_memory();
    }

    pub fn skip_bootrom(&mut self) {
        todo!();
    }
}

impl Default for Nds {
    fn default() -> Self {
        let mut nds = Self {
            cpu7: Cpu::default(),
            cpu9: Cpu::default(),
            ppu: NdsEngines::default(),
            apu: Apu::default(),
            memory: Memory::default(),
            cart: Cartridge::default(),
            dmas: [Dmas::default(), Dmas::default()],
            timers: [Timers::default(), Timers::default()],
            scheduler: Scheduler::default(),
            time_7: 0,
            debugger: Debugger::default(),
            options: EmulateOptions::default(),
            config: SystemConfig::default(),
            ticking: false,
        };

        // ARM9 has a different entry point compared to ARM7.
        nds.cpu9.registers[15] = 0xFFFF_0000;

        // Initialize scheduler
        nds.scheduler.schedule(
            NdsEvent::ApuEvent(ApuEvent::PushSample),
            audio::SAMPLE_EVERY_N_CLOCKS,
        );

        nds
    }
}

pub trait NdsCpu: ArmSystem + DerefMut<Target = Nds> {
    const I: usize;
}

type CpuDevice<T> = [T; 2];

#[repr(transparent)]
struct Nds7(*mut Nds);
#[repr(transparent)]
struct Nds9(*mut Nds);

unsafe impl Send for Nds7 {}
unsafe impl Send for Nds9 {}
