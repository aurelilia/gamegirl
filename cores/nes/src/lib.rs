// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod apu;
mod cartridge;
mod cpu;
mod joypad;
mod memory;
mod ppu;
mod scheduling;

use std::{mem, path::PathBuf};

use apu::Apu;
use cartridge::Cartridge;
use common::{
    common_functions,
    components::{debugger::Debugger, scheduler::Scheduler, storage::GameSave},
    misc::{Button, EmulateOptions, SystemConfig},
    numutil::NumExt,
    produce_samples_buffered, Core,
};
use cpu::Cpu;
use joypad::Joypad;
use memory::Memory;
use ppu::Ppu;
use scheduling::NesEvent;

const CLOCK_HZ: u32 = 1_789_773;

pub type NesDebugger = Debugger<u16>;

/// The system and it's state.
/// Represents the entire console.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Nes {
    pub cpu: Cpu,
    pub mem: Memory,
    pub ppu: Ppu,
    pub apu: Apu,
    pub cart: Cartridge,
    pub joypad: Joypad,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub debugger: NesDebugger,
    scheduler: Scheduler<NesEvent>,

    /// Temporary used by [advance_delta]. Will be true until the scheduled
    /// PauseEmulation event fires.
    ticking: bool,

    /// System config.
    pub config: SystemConfig,
    /// Emulation options.
    pub options: EmulateOptions,
}

impl Core for Nes {
    common_functions!(CLOCK_HZ, NesEvent::PauseEmulation, [256, 240]);
    produce_samples_buffered!(48000);

    fn advance(&mut self) {
        Cpu::exec_next_inst(self);
    }

    fn reset(&mut self) {
        let old_self = mem::take(self);

        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
    }

    fn skip_bootrom(&mut self) {}

    fn set_button(&mut self, btn: Button, pressed: bool) {}

    fn make_save(&self) -> Option<GameSave> {
        todo!();
    }
}

impl Nes {
    /// Advance the scheduler, which controls everything except the CPU.
    fn advance_clock(&mut self, cycles: u16) {
        self.scheduler.advance(cycles.u32());
        while let Some(event) = self.scheduler.get_next_pending() {
            event.kind.dispatch(self, event.late_by);
        }
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
    }

    /// Create a system with a cart already loaded.
    pub fn with_cart(cart: Vec<u8>, _: Option<PathBuf>, _: &SystemConfig) -> Box<Self> {
        let mut nes = Box::<Self>::default();
        nes.cart = Cartridge::from_rom(cart);
        nes
    }
}

impl Default for Nes {
    fn default() -> Self {
        Self {
            cpu: Cpu::default(),
            mem: Memory::default(),
            ppu: Ppu::default(),
            apu: Apu::default(),
            cart: Cartridge::default(),
            joypad: Joypad::default(),
            debugger: Default::default(),
            scheduler: Default::default(),

            ticking: false,
            config: Default::default(),
            options: Default::default(),
        }
    }
}
