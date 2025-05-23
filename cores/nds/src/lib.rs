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

#![no_std]
#![allow(unused)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(if_let_guard)]

extern crate alloc;

mod addr;
mod cpu;
mod graphics;
mod hw;
mod io;
mod memory;
mod scheduling;

use alloc::{boxed::Box, string::String, sync::Weak, vec::Vec};
use core::{
    cell::UnsafeCell,
    mem::{self, MaybeUninit},
    ops::{Deref, DerefMut, Index, IndexMut},
};

use addr::{BIOSPROT, SOUNDBIAS};
use armchair::{interface::Bus, state::Flag, Address, Cpu, Interrupt};
use common::{
    common::options::{EmulateOptions, SystemConfig},
    common_functions,
    components::{
        scheduler::Scheduler,
        storage::{GameCart, GameSave},
    },
    numutil::NumExt,
    Colour, Common, Core, Time, TimeS, UnsafeArc,
};
use cpu::{
    cp15::Cp15,
    math::{Div, Sqrt},
};
use hw::{bios::UserSettings, input::Input, ipc::IpcFifo, spi::SpiBus};
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
    ($name:ident, $idx:expr, $cpu:ident) => {
        /// Wrapper for one of the CPUs.
        /// Raw pointer was chosen to avoid lifetimes.
        #[repr(transparent)]
        pub struct $name(UnsafeArc<NdsInner>);

        impl Deref for $name {
            type Target = Nds;

            #[inline]
            fn deref(&self) -> &Self::Target {
                unsafe { core::mem::transmute(self) }
            }
        }

        impl DerefMut for $name {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { core::mem::transmute(self) }
            }
        }

        impl NdsCpu for $name {
            const I: usize = $idx;
            fn mk(ds: &mut Nds) -> Self {
                Self(ds.0.clone())
            }
            fn cpu(&mut self) -> &mut Cpu<Self> {
                &mut self.$cpu
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

nds_wrapper!(Nds7, 0, cpu7);
nds_wrapper!(Nds9, 1, cpu9);

//#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Nds(UnsafeArc<NdsInner>);

//#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct NdsInner {
    pub cpu7: Cpu<Nds7>,
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
    spi: SpiBus,

    scheduler: Scheduler<NdsEvent>,
    time_7: Time,

    // #[cfg_attr(feature = "serde", serde(skip, default))]
    pub c: Common,
}

impl Core for Nds {
    common_functions!(NDS9_CLOCK, NdsEvent::PauseEmulation, [256, 192 * 2]);

    fn advance(&mut self) {
        // Run the ARM9, then keep running the ARM7
        // until it has caught up
        if self.cpu9.state.is_halted {
            let evt = self.scheduler.pop();
            evt.kind.dispatch(self, evt.late_by);
            self.cpu9.check_unsuspend();
        } else {
            self.cpu9.continue_running();
        }

        if self.cpu7.state.is_halted {
            self.cpu9.check_unsuspend();
        } else {
            while self.time_7 < self.scheduler.now() {
                self.cpu7.continue_running();
            }
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
            let value = self.cart.rom[addr % self.cart.rom.len()];
            self.nds9().set(0x27FFE00 + addr as u32, value)
        }

        // Write binaries and set registers
        let header = self.cart.header();
        {
            for i in 0..header.arm7_size {
                let value = self.cart.rom[header.arm7_offset.us() + i.us()];
                self.cpu7.bus.set(header.arm7_ram_addr + i, value)
            }

            self.cpu7.state.sp[0] = 0x0380_FD80;
            self.cpu7.state.registers[13] = 0x0380_FD80;
            self.cpu7.state.sp[2] = 0x0380_FFC0;
            self.cpu7.state.sp[4] = 0x0380_FF80;
            self.cpu7.state.set_cpsr(0x1F);

            self.cpu7.state.registers[14] = header.arm7_entry_addr;
            self.cpu7.state.registers[15] = header.arm7_entry_addr + 4;
        }
        {
            for i in 0..header.arm9_size {
                let value = self.cart.rom[header.arm9_offset.us() + i.us()];
                self.cpu9.bus.set(header.arm9_ram_addr + i, value)
            }

            self.cpu9.state.sp[0] = 0x0300_2F7C;
            self.cpu9.state.registers[13] = 0x0300_2F7C;
            self.cpu9.state.sp[2] = 0x0300_2FC0;
            self.cpu9.state.sp[4] = 0x0300_2F80;
            self.cpu9.state.set_cpsr(0x1F);

            self.cpu9.state.registers[14] = header.arm9_entry_addr;
            self.cpu9.state.registers[15] = header.arm9_entry_addr + 4;
        }

        // Setup system state
        self.memory.wram_status = WramStatus::All7;
        self.memory.postflg = true;
        self.nds7().set_mmio(BIOSPROT, 0x1204u16);
        self.nds9().set_mmio(SOUNDBIAS, 0x200u16);

        /// Write RAM things
        self.nds9().set::<u32>(0x027FF800, 0x00001FC2);
        self.nds9().set::<u32>(0x027FF804, 0x00001FC2);
        self.nds9().set::<u16>(0x027FF850, 0x5835);
        self.nds9().set::<u16>(0x027FF880, 0x0007);
        self.nds9().set::<u16>(0x027FF884, 0x0006);
        self.nds9().set::<u32>(0x027FFC00, 0x00001FC2);
        self.nds9().set::<u32>(0x027FFC04, 0x00001FC2);
        self.nds9().set::<u16>(0x027FFC10, 0x5835);
        self.nds9().set::<u16>(0x027FFC40, 0x0001);

        // Write user settings
        // TODO
        // let settings = UserSettings::get_bogus();
    }

    fn make_save(&self) -> Option<GameSave> {
        // TODO
        None
    }

    fn get_rom(&self) -> Vec<u8> {
        self.cart.rom.clone()
    }

    fn try_new(cart_ref: &mut Option<GameCart>, config: &SystemConfig) -> Option<Box<Self>> {
        let mut nds = Box::<Self>::default();
        nds.c.config = config.clone();
        if let Some(bios) = config.get_bios("nds7") {
            nds.memory.bios7 = bios.into();
        }
        if let Some(bios) = config.get_bios("nds9") {
            nds.memory.bios9 = bios.into();
        }
        if let Some(fw) = config.get_bios("ndsfw") {
            nds.spi.firm_data = fw.into();
        }

        if let Some(cart) = cart_ref.take() {
            if cart.rom.iter().skip(0x15).take(6).any(|b| *b != 0) {
                // Not NDS cart! Missing zero-filled header region
                *cart_ref = Some(cart);
                return None;
            }
            nds.cart.load_rom(cart.rom);
        }

        nds.init_memory();
        Gpu::init_render(&mut nds);
        Some(nds)
    }
}

impl Nds {
    #[inline]
    pub fn nds7(&mut self) -> &mut Nds7 {
        unsafe { core::mem::transmute(self) }
    }

    #[inline]
    pub fn nds9(&mut self) -> &mut Nds9 {
        unsafe { core::mem::transmute(self) }
    }

    pub fn get_inst_mnemonic<DS: NdsCpu>(ds: &mut DS, ptr: Address) -> String {
        let cpu = ds.cpu();
        let inst = cpu.bus.get(&mut cpu.state, ptr);
        cpu.state.get_inst_mnemonic(inst)
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        // TODO
        // self.c.restore_from(old_self.c);
        // self.init_memory();
    }
}

impl Default for Nds {
    fn default() -> Self {
        let mut uninit: UnsafeArc<MaybeUninit<NdsInner>> = UnsafeArc::new(MaybeUninit::uninit());
        let nds7 = uninit.clone();
        let nds9 = uninit.clone();
        uninit.write(NdsInner {
            cpu7: Cpu::new(unsafe { core::mem::transmute(nds7) }),
            cpu9: Cpu::new(unsafe { core::mem::transmute(nds9) }),
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
            spi: SpiBus::default(),
            scheduler: Scheduler::default(),
            time_7: 0,
            c: Common::default(),
        });
        let mut nds = Nds(unsafe { core::mem::transmute(uninit) });

        // ARM9 has a different entry point compared to ARM7.
        nds.cpu9.state.registers[15] = 0xFFFF_0000;

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
pub trait NdsCpu: Bus + DerefMut<Target = Nds> {
    const I: usize;
    fn mk(ds: &mut Nds) -> Self;
    fn cpu(&mut self) -> &mut Cpu<Self>;
}

/// Type for devices that both CPUs have.
type CpuDevice<T> = [T; 2];

impl core::ops::Deref for Nds {
    type Target = NdsInner;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl core::ops::DerefMut for Nds {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}
