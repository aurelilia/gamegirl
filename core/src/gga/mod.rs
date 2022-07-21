// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::mem;

use audio::Apu;
use cartridge::Cartridge;
use cpu::Cpu;
use memory::Memory;
use serde::{Deserialize, Serialize};

use crate::{
    common::{self, EmulateOptions, SystemConfig},
    components::{debugger::Debugger, scheduler::Scheduler},
    gga::{
        addr::{KEYINPUT, SOUNDBIAS, WAITCNT},
        audio::SAMPLE_EVERY_N_CLOCKS,
        cpu::registers::Flag,
        dma::Dmas,
        graphics::threading::{new_ppu, GgaPpu},
        scheduling::{AdvEvent, ApuEvent, PpuEvent},
        timer::Timers,
    },
    numutil::NumExt,
    Colour,
};

pub mod addr;
mod audio;
mod cartridge;
pub mod cpu;
mod dma;
pub mod graphics;
mod input;
mod memory;
mod scheduling;
mod timer;

#[cfg(not(target_arch = "wasm32"))]
pub mod remote_debugger;

pub const CPU_CLOCK: f32 = 2u32.pow(24) as f32;

pub type GGADebugger = Debugger<u32>;

/// Console struct representing a GGA. Contains all state and is used for system
/// emulation.
#[derive(Deserialize, Serialize)]
pub struct GameGirlAdv {
    pub cpu: Cpu,
    pub memory: Memory,
    pub ppu: GgaPpu,
    pub apu: Apu,
    pub dma: Dmas,
    pub timers: Timers,
    pub cart: Cartridge,

    scheduler: Scheduler<AdvEvent>,
    pub options: EmulateOptions,
    pub config: SystemConfig,

    #[serde(skip)]
    #[serde(default)]
    pub debugger: GGADebugger,
    /// Temporary used by [advance_delta]. Will be true until the scheduled
    /// PauseEmulation event fires.
    ticking: bool,
}

impl GameGirlAdv {
    common_functions!(CPU_CLOCK, AdvEvent::PauseEmulation);

    /// Step forward the emulated console including all subsystems.
    pub fn advance(&mut self) {
        Cpu::continue_running(self);
    }

    /// Advance everything but the CPU by a clock cycle.
    fn advance_clock(&mut self) {
        if self.scheduler.has_events() {
            while let Some(event) = self.scheduler.get_next_pending() {
                event.kind.dispatch(self, event.late_by);
            }
        }
    }

    /// Add S/N cycles, which advance the system besides the CPU.
    #[inline]
    fn add_sn_cycles(&mut self, count: u16) {
        self.scheduler.advance(count.u32());
    }

    /// Add I cycles, which advance the system besides the CPU.
    #[inline]
    fn add_i_cycles(&mut self, count: u16) {
        self.scheduler.advance(count.u32());
        if self.cpu.pc() > 0x800_0000 {
            if self[WAITCNT].is_bit(14) {
                self.memory.prefetch_len += 1;
            } else {
                self.cpu.access_type = Access::NonSeq;
            }
        }
    }

    fn reg(&self, idx: u32) -> u32 {
        self.cpu.reg(idx)
    }

    fn low(&self, idx: u16) -> u32 {
        self.cpu.low(idx)
    }

    pub fn get_inst_mnemonic(&self, ptr: u32) -> String {
        if self.cpu.flag(Flag::Thumb) {
            let inst = self.get_hword(ptr);
            Self::get_mnemonic_thumb(inst)
        } else {
            let inst = self.get_word(ptr);
            Self::get_mnemonic_arm(inst)
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

    pub fn skip_bootrom(&mut self) {
        self.cpu.set_cpsr(0x1F);
        self.set_pc(0x0800_0000);
        self.cpu.sp[1] = 0x0300_7F00;
        self.cpu.sp[3] = 0x0300_7F00;
        self.cpu.sp[5] = 0x0300_7F00;
    }
}

impl Default for GameGirlAdv {
    fn default() -> Self {
        let mut gg = Self {
            cpu: Cpu::default(),
            memory: Memory::default(),
            ppu: new_ppu(),
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
        gg[SOUNDBIAS] = 0x200;

        // Initialize scheduler events
        gg.scheduler
            .schedule(AdvEvent::PpuEvent(PpuEvent::HblankStart), 960);
        Apu::init_scheduler(&mut gg);
        gg.scheduler
            .schedule(AdvEvent::ApuEvent(ApuEvent::Sequencer), 0x8000);
        gg.scheduler.schedule(
            AdvEvent::ApuEvent(ApuEvent::PushSample),
            SAMPLE_EVERY_N_CLOCKS,
        );

        // Initialize DMA
        gg.dma.running = 99;

        gg
    }
}

/// Enum for the types of memory accesses; either sequential
/// or non-sequential. The numbers assigned to the variants are
/// to speed up reading the wait times in `memory.rs`.
#[derive(Copy, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Access {
    Seq = 0,
    NonSeq = 16,
}
