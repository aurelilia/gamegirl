// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(if_let_guard)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(trait_alias)]

use std::{iter, mem, path::PathBuf};

use arm_cpu::{registers::Flag, Cpu};
use audio::{mplayer, Apu};
use cartridge::Cartridge;
use common::{
    common_functions,
    components::{
        debugger::Debugger,
        scheduler::Scheduler,
        storage::{GameSave, Storage},
    },
    misc::{EmulateOptions, SystemConfig},
    numutil::NumExt,
    produce_samples_buffered, Core, TimeS,
};
use cpu::CPU_CLOCK;
use elf_rs::{Elf, ElfFile};
use memory::Memory;
use ppu::Ppu;
use scheduling::PpuEvent;
use serial::Serial;

use crate::{
    dma::Dmas,
    scheduling::{AdvEvent, ApuEvent},
    timer::Timers,
};

pub mod addr;
mod audio;
mod cartridge;
mod cpu;
mod dma;
mod input;
mod memory;
pub mod ppu;
mod scheduling;
mod serial;
pub mod timer;

pub type GGADebugger = Debugger<u32>;

/// Console struct representing a GGA. Contains all state and is used for system
/// emulation.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct GameGirlAdv {
    pub cpu: Cpu<Self>,
    pub memory: Memory,
    pub ppu: Ppu,
    pub apu: Apu,
    pub dma: Dmas,
    pub timers: Timers,
    pub cart: Cartridge,
    pub serial: Serial,

    scheduler: Scheduler<AdvEvent>,
    pub options: EmulateOptions,
    pub config: SystemConfig,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub debugger: GGADebugger,
    /// Temporary used by [advance_delta]. Will be true until the scheduled
    /// PauseEmulation event fires.
    ticking: bool,
}

impl Core for GameGirlAdv {
    common_functions!(CPU_CLOCK, AdvEvent::PauseEmulation, [240, 160]);
    produce_samples_buffered!(2u32.pow(16));

    fn advance(&mut self) {
        if self.cpu.is_halted {
            // We're halted, emulate peripherals until an interrupt is pending
            let evt = self.scheduler.pop();
            evt.kind.dispatch(self, evt.late_by);
            Cpu::check_unsuspend(self);
        } else {
            Cpu::continue_running(self);
        }
    }

    fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
    }

    fn skip_bootrom(&mut self) {
        self.cpu.set_cpsr(0x1F);
        self.cpu.registers[15] = 0x0800_0004;
        self.cpu.sp[0] = 0x0300_7F00;
        self.cpu.sp[2] = 0x0300_7FE0;
        self.cpu.sp[4] = 0x0300_7FA0;
    }

    fn make_save(&self) -> Option<GameSave> {
        self.cart.make_save()
    }

    fn get_memory(&self, addr: usize) -> u8 {
        self.get(addr as u32)
    }

    fn get_registers(&self) -> Vec<usize> {
        self.cpu.registers.into_iter().map(NumExt::us).collect()
    }

    fn get_serial(&self) -> &[u8] {
        unimplemented!("Not implemented for this core")
    }

    fn get_rom(&self) -> Vec<u8> {
        self.cart.rom.clone()
    }
}

impl GameGirlAdv {
    /// Advance everything but the CPU.
    fn advance_clock(&mut self) {
        if self.scheduler.has_events() {
            while let Some(event) = self.scheduler.get_next_pending() {
                event.kind.dispatch(self, event.late_by);
            }
        }
    }

    pub fn get_inst_mnemonic(&self, ptr: u32) -> String {
        if self.cpu.flag(Flag::Thumb) {
            let inst = self.get(ptr);
            Cpu::<Self>::get_mnemonic_thumb(inst)
        } else {
            let inst = self.get(ptr);
            Cpu::<Self>::get_mnemonic_arm(inst)
        }
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        let save = old_self.cart.make_save();
        self.cart.load_rom(old_self.cart.rom);
        if let Some(save) = save {
            self.cart.load_save(save);
        }

        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
        self.init_memory();
        Ppu::init_render(self);
    }

    pub fn with_cart(cart: Vec<u8>, path: Option<PathBuf>, config: &SystemConfig) -> Box<Self> {
        let (mut cart, is_elf) = if let Some(elf_read) = Self::decode_elf(&cart) {
            (elf_read, true)
        } else {
            (cart, false)
        };

        // Paging implementation requires this to prevent reading unallocated memory
        let until_full_page = 0x7FFF - (cart.len() & 0x7FFF);
        cart.extend(iter::repeat(0).take(until_full_page));

        let mut gga = Box::<GameGirlAdv>::default();
        gga.config = config.clone();
        // gga.apu.hle_hook = mplayer::find_mp2k(&cart).unwrap_or(0); TODO still buggy
        gga.cart.load_rom(cart);
        if let Some(save) = Storage::load(path, gga.cart.title()) {
            gga.cart.load_save(save);
        }
        gga.init_memory();
        Ppu::init_render(&mut gga);

        if is_elf {
            gga.skip_bootrom();
        }

        gga
    }

    fn decode_elf(cart: &[u8]) -> Option<Vec<u8>> {
        let elf = Elf::from_bytes(cart).ok()?;
        let mut buf = vec![0; 0x1FF_FFFF];

        for header in elf
            .section_header_iter()
            .filter(|h| (0x800_0000..=0x9FF_FFFF).contains(&h.addr()))
        {
            let dst_offs = header.addr() - 0x800_0000;
            for (src, dst) in header
                .content()
                .unwrap()
                .iter()
                .zip(buf.iter_mut().skip(dst_offs as usize))
            {
                *dst = *src;
            }
        }

        Some(buf)
    }
}

impl Default for GameGirlAdv {
    fn default() -> Self {
        let mut gg = Self {
            cpu: Cpu::default(),
            memory: Memory::default(),
            ppu: Ppu::default(),
            apu: Apu::default(),
            dma: Dmas::default(),
            timers: Timers::default(),
            cart: Cartridge::default(),
            serial: Serial::default(),

            scheduler: Scheduler::default(),
            options: EmulateOptions::default(),
            config: SystemConfig::default(),
            debugger: GGADebugger::default(),
            ticking: true,
        };

        // Initialize scheduler events
        gg.scheduler
            .schedule(AdvEvent::PpuEvent(PpuEvent::HblankStart), 960);
        Apu::init_scheduler(&mut gg);
        gg.scheduler
            .schedule(AdvEvent::ApuEvent(ApuEvent::Sequencer), 0x8000);
        gg.scheduler
            .schedule(AdvEvent::UpdateKeypad, (CPU_CLOCK / 120.0) as TimeS);

        // Initialize various IO devices
        gg.dma.running = 99;
        gg.apu.bias = 0x200.into();

        gg
    }
}
