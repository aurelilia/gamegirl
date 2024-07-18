// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![allow(unused)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(if_let_guard)]

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
    path::PathBuf,
};

use arm_cpu::{interface::ArmSystem, Cpu};
use common::{
    common::options::{EmulateOptions, SystemConfig},
    common_functions,
    components::{scheduler::Scheduler, storage::GameSave},
    numutil::NumExt,
    Colour, Common, Core, Time,
};
use cpu::{
    cp15::Cp15,
    math::{Div, Sqrt},
};

use crate::{
    audio::Apu,
    cartridge::Cartridge,
    cpu::NDS9_CLOCK,
    dma::Dmas,
    graphics::Gpu,
    memory::Memory,
    scheduling::{ApuEvent, NdsEvent},
    timer::Timers,
};

/// Macro for creating a wrapper of the system, specifically with
/// the use case of being able to implement ARM CPU support twice,
/// since the NDS has 2 CPUs.
macro_rules! nds_wrapper {
    ($name:ident, $idx:expr) => {
        /// Wrapper for one of the CPUs.
        /// Raw pointer was chosen to avoid lifetimes.
        #[repr(transparent)]
        struct $name(*mut Nds);

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

        impl NdsCpu for $name {
            const I: usize = $idx;
        }

        unsafe impl Send for $name {}

        // Satisfy serde...
        impl Default for $name {
            fn default() -> $name {
                unreachable!()
            }
        }
    };
}

nds_wrapper!(Nds7, 0);
nds_wrapper!(Nds9, 1);

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Nds {
    cpu7: Cpu<Nds7>,
    cpu9: Cpu<Nds9>,
    cp15: Cp15,
    div: Div,
    sqrt: Sqrt,

    gpu: Gpu,
    apu: Apu,
    memory: Memory,
    pub cart: Cartridge,
    dmas: CpuDevice<Dmas>,
    timers: CpuDevice<Timers>,

    scheduler: Scheduler<NdsEvent>,
    time_7: Time,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub c: Common,
}

impl Core for Nds {
    common_functions!(NDS9_CLOCK, NdsEvent::PauseEmulation, [240, 160 * 2]);

    fn advance(&mut self) {
        // Run an instruction on the ARM9, then keep running the ARM7
        // until it has caught up
        Cpu::continue_running(&mut self.nds9());
        let mut nds7 = self.nds7();
        while self.time_7 < self.scheduler.now() {
            Cpu::continue_running(&mut nds7);
        }
    }

    fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
    }

    fn skip_bootrom(&mut self) {
        /// Really HLE init on NDS
        for addr in 0..0x200 {
            self.nds9()
                .set(0x27FFE00 + addr as u32, self.cart.rom[addr])
        }
    }

    fn make_save(&self) -> Option<GameSave> {
        todo!();
    }

    fn get_rom(&self) -> Vec<u8> {
        self.cart.rom.clone()
    }
}

impl Nds {
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
        self.c.restore_from(old_self.c);
        self.init_memory();
    }

    pub fn with_cart(cart: Vec<u8>, _path: Option<PathBuf>, config: &SystemConfig) -> Box<Self> {
        let mut nds = Box::<Self>::default();
        nds.c.config = config.clone();
        nds.memory.bios7 = config.get_bios("nds7").unwrap().into();
        nds.memory.bios9 = config.get_bios("nds9").unwrap().into();
        nds.cart.load_rom(cart);
        nds.init_memory();
        nds.skip_bootrom();
        nds
    }
}

impl Default for Nds {
    fn default() -> Self {
        let mut nds = Self {
            cpu7: Cpu::default(),
            cpu9: Cpu::default(),
            cp15: Cp15::default(),
            div: Div::default(),
            sqrt: Sqrt::default(),
            gpu: Gpu::default(),
            apu: Apu::default(),
            memory: Memory::default(),
            cart: Cartridge::default(),
            dmas: [Dmas::default(), Dmas::default()],
            timers: [Timers::default(), Timers::default()],
            scheduler: Scheduler::default(),
            time_7: 0,
            c: Common::default(),
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

/// Trait for things that need to operate on a single CPU,
/// like a DMA or timer.
/// I = 0 for the ARM7, I = 1 for the ARM9;
/// things separated by CPU generally use CpuDevice for easy
/// access with I.
pub trait NdsCpu: ArmSystem + DerefMut<Target = Nds> {
    const I: usize;
}

/// Type for devices that both CPUs have.
type CpuDevice<T> = [T; 2];
