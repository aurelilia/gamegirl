// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(if_let_guard)]

use std::{iter, mem, path::PathBuf};

use arm_cpu::{registers::Flag, Cpu};
use audio::Apu;
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

use crate::{
    addr::{IE, IF, KEYINPUT},
    dma::Dmas,
    scheduling::{AdvEvent, ApuEvent},
    timer::Timers,
};

pub mod addr;
mod audio;
mod cartridge;
mod cpu;
mod dma;
// pub mod graphics;
mod input;
mod memory;
mod ppu;
mod scheduling;
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
            self.cpu.is_halted = (self[IE] & self[IF]) == 0;
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
        self.cpu.registers[15] = 0x0800_0000;
        self.cpu.sp[1] = 0x0300_7F00;
        self.cpu.sp[3] = 0x0300_7F00;
        self.cpu.sp[5] = 0x0300_7F00;
    }

    fn make_save(&self) -> Option<GameSave> {
        self.cart.make_save()
    }

    fn get_memory(&self, addr: usize) -> u8 {
        self.get_byte(addr as u32)
    }

    fn get_registers(&self) -> Vec<usize> {
        self.cpu.registers.into_iter().map(NumExt::us).collect()
    }

    fn get_serial(&self) -> &[u8] {
        unimplemented!("Not implemented for this core")
    }
}

impl GameGirlAdv {
    /// Advance everything but the CPU by a clock cycle.
    fn advance_clock(&mut self) {
        if self.scheduler.has_events() {
            while let Some(event) = self.scheduler.get_next_pending() {
                event.kind.dispatch(self, event.late_by);
            }
        }
    }

    pub fn get_inst_mnemonic(&self, ptr: u32) -> String {
        if self.cpu.flag(Flag::Thumb) {
            let inst = self.get_hword(ptr);
            Cpu::<Self>::get_mnemonic_thumb(inst)
        } else {
            let inst = self.get_word(ptr);
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
        gga.cart.load_rom(cart);
        if let Some(save) = Storage::load(path, gga.cart.title()) {
            gga.cart.load_save(save);
        }
        gga.init_memory();

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

            scheduler: Scheduler::default(),
            options: EmulateOptions::default(),
            config: SystemConfig::default(),
            debugger: GGADebugger::default(),
            ticking: true,
        };

        // Initialize various IO registers
        gg[KEYINPUT] = 0x3FF;
        gg.apu.bias = 0x200.into();

        // Initialize scheduler events
        gg.scheduler
            .schedule(AdvEvent::PpuEvent(PpuEvent::HblankStart), 960);
        Apu::init_scheduler(&mut gg);
        gg.scheduler
            .schedule(AdvEvent::ApuEvent(ApuEvent::Sequencer), 0x8000);
        gg.scheduler
            .schedule(AdvEvent::UpdateKeypad, (CPU_CLOCK / 120.0) as TimeS);

        // Initialize DMA
        gg.dma.running = 99;

        gg
    }
}
