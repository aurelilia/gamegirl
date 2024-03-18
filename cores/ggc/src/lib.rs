// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{mem, path::PathBuf};

use common::{
    common_functions,
    components::{
        debugger::Debugger,
        memory::MemoryMapper,
        scheduler::Scheduler,
        storage::{GameSave, Storage},
    },
    misc::{Button, EmulateOptions, SystemConfig},
    numutil::NumExt,
    produce_samples_buffered, Core,
};
use io::addr::DIV;

use crate::{
    cpu::{Cpu, Interrupt},
    io::{
        addr::{BOOTROM_DISABLE, IF, KEY1},
        apu::Apu,
        cartridge::Cartridge,
        dma::Hdma,
        joypad::Joypad,
        ppu::Ppu,
        scheduling::GGEvent,
        timer::Timer,
        Memory,
    },
};

pub mod cpu;
pub mod io;

const T_CLOCK_HZ: u32 = 4_194_304;

pub type GGDebugger = Debugger<u16>;

/// The system and it's state.
/// Represents the entire console.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct GameGirl {
    pub cpu: Cpu,
    pub mem: Memory,

    pub cgb: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub debugger: GGDebugger,
    scheduler: Scheduler<GGEvent>,

    pub cart: Cartridge,
    pub timer: Timer,
    pub ppu: Ppu,
    pub joypad: Joypad,
    pub apu: Apu,
    pub dma: u8,
    pub hdma: Hdma,

    /// CPU speed, 1x or 2x.
    speed: u8,
    /// Shift of m-cycles to t-clocks, which is different in CGB double speed
    /// mode. Regular: 2, CGB 2x: 1.
    t_shift: u8,
    /// Temporary used by [advance_delta]. Will be true until the scheduled
    /// PauseEmulation event fires.
    ticking: bool,

    /// System config.
    pub config: SystemConfig,
    /// Emulation options.
    pub options: EmulateOptions,
}

impl Core for GameGirl {
    common_functions!(T_CLOCK_HZ, GGEvent::PauseEmulation, [160, 144]);
    produce_samples_buffered!(48000);

    fn advance(&mut self) {
        Cpu::exec_next_inst(self);
    }

    fn reset(&mut self) {
        let old_self = mem::take(self);
        let save = old_self.cart.make_save();
        self.load_cart_mem(old_self.cart, &old_self.config);
        if let Some(save) = save {
            self.cart.load_save(save);
        }

        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
        MemoryMapper::init_pages(self);
    }

    fn skip_bootrom(&mut self) {
        self.cpu.pc = 0x100;
        self.set(BOOTROM_DISABLE, 1u8);
    }

    fn set_button(&mut self, btn: Button, pressed: bool) {
        Joypad::set(self, btn, pressed);
    }

    fn make_save(&self) -> Option<GameSave> {
        self.cart.make_save()
    }

    fn get_memory(&self, addr: usize) -> u8 {
        self.get(addr as u16)
    }

    fn get_registers(&self) -> Vec<usize> {
        self.cpu.regs.iter().map(|r| *r as usize).collect()
    }

    fn get_serial(&self) -> &[u8] {
        self.debugger.serial_output.as_bytes()
    }
}

impl GameGirl {
    /// Advance the scheduler, which controls everything except the CPU.
    fn advance_clock(&mut self, m_cycles: u16) {
        self.scheduler.advance((m_cycles << self.t_shift).u32());
        while let Some(event) = self.scheduler.get_next_pending() {
            event.kind.dispatch(self, event.late_by);
        }

        for _ in 0..m_cycles {
            Timer::step(self);
            self.apu.clock(self.t_shift == 1, Timer::read(self, DIV))
        }
    }

    /// Switch between CGB 2x and normal speed mode.
    fn switch_speed(&mut self) {
        self.t_shift = if self.t_shift == 2 { 1 } else { 2 };
        self.speed = if self.t_shift == 1 { 2 } else { 1 };
        self[KEY1] = (self.speed - 1) << 7;

        for _ in 0..64 {
            self.advance_clock(2048 / 64);
        }
    }

    /// Request an interrupt. Sets the bit in IF.
    fn request_interrupt(&mut self, ir: Interrupt) {
        self[IF] = self[IF].set_bit(ir.to_index(), true);
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        let save = old_self.cart.make_save();
        self.cart.rom = old_self.cart.rom;
        if let Some(save) = save {
            self.cart.load_save(save);
        }

        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
        MemoryMapper::init_pages(self);
    }

    /// Load the given cartridge.
    /// `reset` indicates if the system should be reset before loading.
    pub fn load_cart(&mut self, cart: Cartridge, config: &SystemConfig, reset: bool) {
        if reset {
            let old_self = mem::take(self);
            self.debugger = old_self.debugger;
        }
        self.load_cart_mem(cart, config);
        self.config = config.clone();
    }

    /// Create a system with a cart already loaded.
    pub fn with_cart(cart: Vec<u8>, path: Option<PathBuf>, config: &SystemConfig) -> Box<Self> {
        let mut cart = Cartridge::from_rom(cart);
        if let Some(save) = Storage::load(path, cart.title(true)) {
            cart.load_save(save);
        }

        let mut ggc = Box::<Self>::default();
        ggc.load_cart(cart, config, false);
        ggc
    }
}

impl Default for GameGirl {
    fn default() -> Self {
        let debugger = GGDebugger::default();
        Self {
            cpu: Cpu::default(),
            mem: Memory::new(),
            config: SystemConfig::default(),

            cgb: false,
            debugger,
            scheduler: Scheduler::default(),

            timer: Timer::default(),
            ppu: Ppu::new(),
            joypad: Joypad::default(),
            apu: Apu::new(false),
            dma: 0,
            hdma: Hdma::default(),
            cart: Cartridge::dummy(),

            speed: 1,
            t_shift: 2,
            ticking: true,
            options: EmulateOptions::default(),
        }
    }
}
