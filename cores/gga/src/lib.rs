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

use std::{cmp::Ordering, mem, path::PathBuf};

use arm_cpu::Cpu;
use audio::Apu;
use common::{
    common::{
        debugger::{self, Width},
        options::SystemConfig,
        Common,
    },
    common_functions,
    components::{scheduler::Scheduler, storage::GameSave, thin_pager::ThinPager},
    numutil::NumExt,
    Core, TimeS,
};
use cpu::CPU_CLOCK;
use elf_rs::{Elf, ElfFile};
use hw::{cartridge::Cartridge, serial::Serial};
use memory::Memory;
use ppu::Ppu;
use scheduling::PpuEvent;

use crate::{
    hw::{dma::Dmas, timer::Timers},
    scheduling::AdvEvent,
};

pub mod addr;
mod audio;
mod cpu;
pub mod hw;
mod io;
mod memory;
pub mod ppu;
mod scheduling;

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
    pub c: Common,
}

impl Core for GameGirlAdv {
    common_functions!(CPU_CLOCK, AdvEvent::PauseEmulation, [240, 160]);

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

    fn get_memory(&self, addr: u32, width: Width) -> u32 {
        self.get::<u32>(addr) & width.mask()
    }

    fn search_memory(&self, value: u32, width: Width, kind: Ordering) -> Vec<u32> {
        let mut values = Vec::new();
        debugger::search_array(
            &mut values,
            &self.memory.iwram,
            0x200_0000,
            value,
            width,
            kind,
        );
        debugger::search_array(
            &mut values,
            &self.memory.ewram,
            0x300_0000,
            value,
            width,
            kind,
        );
        debugger::search_array(
            &mut values,
            &self.ppu.palette,
            0x500_0000,
            value,
            width,
            kind,
        );
        debugger::search_array(&mut values, &self.ppu.vram, 0x600_0000, value, width, kind);
        debugger::search_array(&mut values, &self.ppu.oam, 0x700_0000, value, width, kind);
        debugger::search_array(&mut values, &self.cart.ram, 0xE00_0000, value, width, kind);
        values
    }

    fn get_registers(&self) -> Vec<usize> {
        self.cpu.registers.into_iter().map(NumExt::us).collect()
    }

    fn get_rom(&self) -> Vec<u8> {
        self.cart.rom.clone()
    }

    fn set_memory(&mut self, addr: u32, value: u32, width: Width) {
        match width {
            Width::Byte => self.set(addr, value.u8()),
            Width::Halfword => self.set(addr, value.u16()),
            Width::Word => self.set(addr, value),
        }
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

    pub fn get_inst_mnemonic(&mut self, ptr: u32) -> String {
        Cpu::<Self>::get_inst(self, ptr)
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        let save = old_self.cart.make_save();
        self.cart.load_rom(old_self.cart.rom);
        if let Some(save) = save {
            self.cart.load_save(save);
        }

        self.c.restore_from(old_self.c);
        self.setup_host_state();
    }

    pub fn setup_host_state(&mut self) {
        self.init_memory();
        Ppu::init_render(self);
        if let Some(bios) = self.c.config.get_bios("agb") {
            self.memory.bios = bios.into();
        }
    }

    pub fn initialize(&mut self) {
        Apu::init_scheduler(self);
        self.scheduler
            .schedule(AdvEvent::PpuEvent(PpuEvent::HblankStart), 960);
        self.scheduler
            .schedule(AdvEvent::UpdateKeypad, (CPU_CLOCK / 120.0) as TimeS);
        self.c.audio_buffer.set_input_sr(2usize.pow(15));
        self.setup_host_state();
        // self.apu.hle_hook = mplayer::find_mp2k(&cart).unwrap_or(0); TODO
        // still buggy
    }

    pub fn new(cart: Option<Vec<u8>>, path: Option<PathBuf>, config: &SystemConfig) -> Box<Self> {
        let cart = if let Some(cart) = cart {
            let mut cart = if let Some(elf_read) = Self::decode_elf(&cart) {
                elf_read
            } else {
                cart
            };
            ThinPager::normalize(&mut cart);
            Cartridge::with_rom_and_stored_ram(cart, path)
        } else {
            Cartridge::default()
        };

        let mut gg = Box::<GameGirlAdv>::new(GameGirlAdv {
            cpu: Cpu::default(),
            memory: Memory::default(),
            ppu: Ppu::default(),
            apu: Apu::default(),
            dma: Dmas::default(),
            timers: Timers::default(),
            cart,
            serial: Serial::default(),

            scheduler: Scheduler::default(),
            c: Common::with_config(config.clone()),
        });
        gg.initialize();
        gg
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
            c: Common::default(),
        };
        gg.initialize();
        gg
    }
}
