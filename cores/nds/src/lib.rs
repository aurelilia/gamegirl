// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

// Things left to do:
// - IPC FIFO implementation
// - Video stuff
// - Audio
// - Math registers implementation
// - Link port
// - Touchscreen
// - RTC
// - SPI
// - Power management
// - Cartridge

#![allow(unused)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(if_let_guard)]

mod addr;
mod cpu;
mod graphics;
mod hw;
mod io;
mod memory;
mod scheduling;

use std::{
    mem,
    ops::{Deref, DerefMut, Index, IndexMut},
    path::PathBuf,
};

use arm_cpu::{interface::ArmSystem, registers::Flag, Cpu};
use common::{
    common::options::{EmulateOptions, SystemConfig},
    common_functions,
    components::{scheduler::Scheduler, storage::GameSave},
    numutil::NumExt,
    Colour, Common, Core, Time, TimeS,
};
use cpu::{
    cp15::Cp15,
    math::{Div, Sqrt},
};
use hw::{input::Input, ipc::IpcFifo};
use memory::WramStatus;
use scheduling::PpuEvent;

use crate::{
    cpu::NDS9_CLOCK,
    graphics::Gpu,
    hw::{audio::Apu, cartridge::Cartridge, dma::Dmas, timer::Timers},
    memory::Memory,
    scheduling::{ApuEvent, NdsEvent},
};

/// Macro for creating a wrapper of the system, specifically with
/// the use case of being able to implement ARM CPU support twice,
/// since the NDS has 2 CPUs.
macro_rules! nds_wrapper {
    ($name:ident, $idx:expr) => {
        /// Wrapper for one of the CPUs.
        /// Raw pointer was chosen to avoid lifetimes.
        #[repr(transparent)]
        pub struct $name(*mut Nds);

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
    pub cpu9: Cpu<Nds9>,
    cp15: Cp15,
    div: Div,
    sqrt: Sqrt,
    fifo: IpcFifo,

    gpu: Gpu,
    apu: Apu,
    memory: Memory,
    pub cart: Cartridge,
    dmas: CpuDevice<Dmas>,
    timers: CpuDevice<Timers>,
    input: Input,

    scheduler: Scheduler<NdsEvent>,
    time_7: Time,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub c: Common,
}

impl Core for Nds {
    common_functions!(NDS9_CLOCK, NdsEvent::PauseEmulation, [256, 192 * 2]);

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
        // Write out header
        for addr in 0..0x200 {
            self.nds9().set(
                0x27FFE00 + addr as u32,
                self.cart.rom[addr % self.cart.rom.len()],
            )
        }

        // Write binaries and set registers
        let header = self.cart.header();
        {
            let mut ds = self.nds7();
            for i in 0..header.arm7_size {
                ds.set(
                    header.arm7_entry_addr + i,
                    self.cart.rom[header.arm7_offset.us() + i.us()],
                )
            }

            ds.cpu().sp[0] = 0x0380_FD80;
            ds.cpu().sp[2] = 0x0380_FFC0;
            ds.cpu().sp[4] = 0x0380_FF80;
            ds.cpu().set_cpsr(0x1F);

            ds.cpu().registers[14] = header.arm7_entry_addr;
            ds.cpu().registers[15] = header.arm7_entry_addr + 4;
        }
        {
            let mut ds = self.nds9();
            for i in 0..header.arm9_size {
                ds.set(
                    header.arm9_entry_addr + i,
                    self.cart.rom[header.arm9_offset.us() + i.us()],
                )
            }

            ds.cpu().sp[0] = 0x0300_2F7C;
            ds.cpu().sp[2] = 0x0300_2FC0;
            ds.cpu().sp[4] = 0x0300_2F80;
            ds.cpu().set_cpsr(0x1F);

            ds.cpu().registers[14] = header.arm9_entry_addr;
            ds.cpu().registers[15] = header.arm9_entry_addr + 4;
        }

        // Setup system state
        self.memory.wram_status = WramStatus::All7;
    }

    fn make_save(&self) -> Option<GameSave> {
        // TODO
        None
    }

    fn get_rom(&self) -> Vec<u8> {
        self.cart.rom.clone()
    }
}

impl Nds {
    #[inline]
    pub fn nds7(&mut self) -> Nds7 {
        Nds7(self as *mut Nds)
    }

    #[inline]
    pub fn nds9(&mut self) -> Nds9 {
        Nds9(self as *mut Nds)
    }

    pub fn get_inst_mnemonic<DS: NdsCpu>(ds: &mut DS, ptr: u32) -> String {
        Cpu::<DS>::get_inst(ds, ptr)
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
        if let Some(bios) = config.get_bios("nds7") {
            nds.memory.bios7 = bios.into();
        }
        if let Some(bios) = config.get_bios("nds9") {
            nds.memory.bios9 = bios.into();
        }
        nds.cart.load_rom(cart);

        log::error!("{:#?}", nds.cart.header());
        nds.init_memory();
        Gpu::init_render(&mut nds);

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
            fifo: IpcFifo::default(),
            gpu: Gpu::default(),
            apu: Apu::default(),
            input: Input::default(),
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
            hw::audio::SAMPLE_EVERY_N_CLOCKS,
        );
        nds.scheduler
            .schedule(NdsEvent::PpuEvent(PpuEvent::HblankStart), 3072);
        nds.scheduler
            .schedule(NdsEvent::UpdateKeypad, (NDS9_CLOCK as f64 / 120.0) as TimeS);

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
