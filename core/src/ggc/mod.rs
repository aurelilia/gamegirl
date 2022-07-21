// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::mem;

use serde::{Deserialize, Serialize};

use crate::{
    common::{self, EmulateOptions, SystemConfig},
    debugger::Debugger,
    ggc::{
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
    },
    numutil::NumExt,
    scheduler::Scheduler,
    Colour,
};

pub mod cpu;
pub mod io;

const T_CLOCK_HZ: u32 = 4_194_304;

pub type GGDebugger = Debugger<u16>;

/// The system and it's state.
/// Represents the entire console.
#[derive(Deserialize, Serialize)]
pub struct GameGirl {
    pub cpu: Cpu,
    pub mem: Memory,

    cgb: bool,
    #[serde(skip)]
    #[serde(default)]
    pub debugger: GGDebugger,
    scheduler: Scheduler<GGEvent>,

    pub cart: Cartridge,
    pub timer: Timer,
    pub ppu: Ppu,
    pub joypad: Joypad,
    pub apu: Apu,
    pub hdma: Hdma,

    /// CPU speed, 1/2x.
    speed: u8,
    /// Shift of m-cycles to t-clocks, which is different in CGB double speed
    /// mode. Regular: 2, CGB 2x: 1.
    t_shift: u8,
    /// Temporary used by [advance_delta]. Will be true until the scheduled
    /// PauseEmulation event fires,
    ticking: bool,

    /// System config.
    pub config: SystemConfig,
    /// Emulation options.
    pub options: EmulateOptions,
}

impl GameGirl {
    /// Advance the system clock by the given delta in seconds.
    /// Might advance a few clocks more; especially if a GDMA transfer
    /// occurs at the wrong time.
    pub fn advance_delta(&mut self, delta: f32) {
        if !self.options.running {
            return;
        }

        let target = (T_CLOCK_HZ as f32 * delta * self.options.speed_multiplier as f32) as i32;
        self.scheduler.schedule(GGEvent::PauseEmulation, target);

        self.ticking = true;
        while self.options.running && self.ticking {
            self.advance();
        }
    }

    /// Step until the PPU has finished producing the current frame.
    /// Only used for rewinding since it causes audio desync very easily.
    pub fn produce_frame(&mut self) -> Option<Vec<Colour>> {
        while self.options.running && self.ppu.last_frame == None {
            self.advance();
        }
        self.ppu.last_frame.take()
    }

    /// Produce the next audio samples and write them to the given buffer.
    /// Writes zeroes if the system is not currently running
    /// and no audio should be played.
    pub fn produce_samples(&mut self, samples: &mut [f32]) {
        if !self.options.running {
            samples.fill(0.0);
            return;
        }

        let target = samples.len() * self.options.speed_multiplier;
        while self.apu.buffer.len() < target {
            if !self.options.running {
                samples.fill(0.0);
                return;
            }
            self.advance();
        }

        let mut buffer = mem::take(&mut self.apu.buffer);
        if self.options.invert_audio_samples {
            // If rewinding, truncate and get rid of any excess samples to prevent
            // audio samples getting backed up
            for (src, dst) in buffer.into_iter().zip(samples.iter_mut().rev()) {
                *dst = src * self.config.volume;
            }
        } else {
            // Otherwise, store any excess samples back in the buffer for next time
            // while again not storing too many to avoid backing up.
            // This way can cause clipping if the console produces audio too fast,
            // however this is preferred to audio falling behind and eating
            // a lot of memory.
            for sample in buffer.drain(target..) {
                self.apu.buffer.push(sample);
            }
            self.apu.buffer.truncate(5_000);

            for (src, dst) in buffer
                .into_iter()
                .step_by(self.options.speed_multiplier)
                .zip(samples.iter_mut())
            {
                *dst = src * self.config.volume;
            }
        }
    }

    /// Advance the system by a single CPU instruction.
    pub fn advance(&mut self) {
        Cpu::exec_next_inst(self);
    }

    /// Advance the scheduler, which controls everything except the CPU.
    fn advance_clock(&mut self, m_cycles: u16) {
        self.scheduler.advance((m_cycles << self.t_shift).u32());
        while let Some(event) = self.scheduler.get_next_pending() {
            event.kind.dispatch(self, event.late_by);
        }
    }

    /// Switch between CGB 2x and normal speed mode.
    fn switch_speed(&mut self) {
        self.t_shift = if self.t_shift == 2 { 1 } else { 2 };
        self.speed = if self.t_shift == 1 { 2 } else { 1 };
        self[KEY1] = (self.t_shift & 1) << 7;

        for _ in 0..16 {
            self.advance_clock(2048 / 16);
        }
    }

    /// Request an interrupt. Sets the bit in IF.
    fn request_interrupt(&mut self, ir: Interrupt) {
        self[IF] = self[IF].set_bit(ir.to_index(), true) as u8;
    }

    /// Restore state after a savestate load. `old_self` should be the
    /// system state before the state was loaded.
    pub fn restore_from(&mut self, old_self: Self) {
        let save = old_self.cart.make_save();
        self.load_cart_mem(old_self.cart, &old_self.config);
        if let Some(save) = save {
            self.cart.load_save(save);
        }

        self.options = old_self.options;
        self.config = old_self.config;
        self.debugger = old_self.debugger;
        self.mem.bootrom = old_self.mem.bootrom;
        self.init_memory();
    }

    /// Reset the console, while keeping the current cartridge inserted.
    pub fn reset(&mut self) {
        let old_self = mem::take(self);
        self.restore_from(old_self);
    }

    /// Create a save state that can be loaded with [load_state].
    pub fn save_state(&self) -> Vec<u8> {
        common::serialize(self, self.config.compress_savestates)
    }

    /// Load a state produced by [save_state].
    /// Will restore the current cartridge and debugger.
    pub fn load_state(&mut self, state: &[u8]) {
        if cfg!(target_arch = "wasm32") {
            // Currently crashes...
            return;
        }

        let old_self = mem::replace(
            self,
            common::deserialize(state, self.config.compress_savestates),
        );
        self.restore_from(old_self);
    }

    /// Load the given cartridge.
    /// `reset` indicates if the system should be reset before loading.
    pub fn load_cart(&mut self, cart: Cartridge, config: &SystemConfig, reset: bool) {
        if reset {
            let old_self = mem::take(self);
            self.debugger = old_self.debugger;
            self.options.frame_finished = old_self.options.frame_finished;
        }
        self.load_cart_mem(cart, config);
        self.config = config.clone();
    }

    /// Create a system with a cart already loaded.
    pub fn with_cart(rom: Vec<u8>) -> Self {
        let mut gg = Self::default();
        gg.load_cart(Cartridge::from_rom(rom), &SystemConfig::default(), false);
        gg.options.running = true;
        gg.options.rom_loaded = true;
        gg
    }

    pub fn skip_bootrom(&mut self) {
        self.cpu.pc = 0x100;
        self.set8(BOOTROM_DISABLE, 1);
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
            hdma: Hdma::default(),
            cart: Cartridge::dummy(),

            speed: 1,
            t_shift: 2,
            ticking: true,
            options: EmulateOptions::default(),
        }
    }
}
