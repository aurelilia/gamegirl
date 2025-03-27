// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![no_std]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::{cmp::Ordering, mem};

use common::{
    common::{
        debugger::{self, Width},
        options::SystemConfig,
    },
    common_functions,
    components::{
        memory_mapper::MemoryMapper,
        scheduler::Scheduler,
        storage::{GameCart, GameSave},
    },
    numutil::{hword, word, NumExt},
    Common, Core, Time,
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

/// The system and it's state.
/// Represents the entire console.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct GameGirl {
    pub cpu: Cpu,
    pub mem: Memory,
    pub cgb: bool,
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

    pub c: Common,
}

impl Core for GameGirl {
    common_functions!(T_CLOCK_HZ, GGEvent::PauseEmulation, [160, 144]);

    fn advance(&mut self) {
        Cpu::exec_next_inst(self);
    }

    fn reset(&mut self) {
        let old_self = mem::take(self);
        let save = old_self.cart.make_save();
        self.load_cart_mem(old_self.cart, &old_self.c.config);
        if let Some(save) = save {
            self.cart.load_save(save);
        }

        self.c.restore_from(old_self.c);
        MemoryMapper::init_pages(self);
    }

    fn skip_bootrom(&mut self) {
        self.cpu.pc = 0x100;
        self.set(BOOTROM_DISABLE, 1u8);
    }

    fn make_save(&self) -> Option<GameSave> {
        self.cart.make_save()
    }

    fn get_memory(&self, addr: u32, width: Width) -> u32 {
        match width {
            Width::Byte => self.get(addr.u16()),
            Width::Halfword => hword(self.get(addr.u16()), self.get(addr.u16() + 1)).u32(),
            Width::Word => word(
                hword(self.get(addr.u16()), self.get(addr.u16() + 1)),
                hword(self.get(addr.u16() + 2), self.get(addr.u16() + 3)),
            ),
        }
    }

    fn search_memory(&self, value: u32, width: Width, kind: Ordering) -> Vec<u32> {
        let mut values = Vec::new();
        debugger::search_array(&mut values, &self.mem.vram, 0x8000, value, width, kind);
        debugger::search_array(&mut values, &self.mem.wram, 0xC000, value, width, kind);
        debugger::search_array(&mut values, &self.mem.oam, 0xFE00, value, width, kind);
        debugger::search_array(&mut values, &self.mem.high, 0xFF00, value, width, kind);
        values
    }

    fn get_registers(&self) -> Vec<usize> {
        self.cpu.regs.iter().map(|r| *r as usize).collect()
    }

    fn get_rom(&self) -> Vec<u8> {
        self.cart.rom.clone()
    }

    fn try_new(cart_ref: &mut Option<GameCart>, config: &SystemConfig) -> Option<Box<Self>> {
        let mut ggc = Box::<Self>::default();
        if let Some(cart) = cart_ref.take() {
            if cart.rom[0x0104] != 0xCE || cart.rom[0x0105] != 0xED {
                // Missing nintendo logo bytes!
                *cart_ref = Some(cart);
                return None;
            }
            let mut cartridge = Cartridge::from_rom(cart.rom);
            if let Some(save) = cart.save {
                cartridge.load_save(save);
            }
            ggc.load_cart(cartridge, config, false);
        }
        Some(ggc)
    }
}

impl GameGirl {
    /// Advance the scheduler, which controls everything except the CPU.
    fn advance_clock(&mut self, m_cycles: u16) {
        self.scheduler.advance((m_cycles << self.t_shift) as Time);
        while let Some(event) = self.scheduler.get_next_pending() {
            event.kind.dispatch(self, event.late_by);
        }

        for _ in 0..m_cycles {
            Timer::step(self);
            self.apu.clock(
                self.t_shift == 1,
                Timer::read(self, DIV),
                &mut self.c.audio_buffer.input,
            )
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

        self.c.restore_from(old_self.c);
        MemoryMapper::init_pages(self);
    }

    /// Load the given cartridge.
    /// `reset` indicates if the system should be reset before loading.
    pub fn load_cart(&mut self, cart: Cartridge, config: &SystemConfig, reset: bool) {
        if reset {
            let old_self = mem::take(self);
            self.c.debugger = old_self.c.debugger;
        }
        self.load_cart_mem(cart, config);
        self.c.config = config.clone();
    }
}

impl Default for GameGirl {
    fn default() -> Self {
        Self {
            cpu: Cpu::default(),
            mem: Memory::new(),

            cgb: false,
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

            c: Common::default(),
        }
    }
}
