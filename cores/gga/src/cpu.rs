// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use arm_cpu::{
    interface::{ArmSystem, RwType},
    Access, Cpu, Exception,
};
use common::{components::debugger::Debugger, numutil::NumExt, Time};

use crate::{addr, audio::mplayer::MusicPlayer, GameGirlAdv};

pub const CPU_CLOCK: f32 = 2u32.pow(24) as f32;

impl ArmSystem for GameGirlAdv {
    const IS_V5: bool = false;
    const IF_ADDR: u32 = addr::IF;

    fn cpur(&self) -> &Cpu<Self> {
        &self.cpu
    }

    fn cpu(&mut self) -> &mut Cpu<Self> {
        &mut self.cpu
    }

    fn advance_clock(&mut self) {
        self.advance_clock();
    }

    fn add_sn_cycles(&mut self, cycles: u16) {
        self.scheduler.advance(cycles as Time);
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.scheduler.advance(cycles as Time);
        if self.cpu.pc() > 0x800_0000 {
            if self.memory.waitcnt.is_bit(14) {
                self.memory.prefetch_len += 1;
            } else {
                self.cpu.access_type = Access::NonSeq;
            }
        }
    }

    fn exception_happened(&mut self, kind: Exception) {
        match kind {
            Exception::Irq if self.cpu.pc() > 0x100_0000 => self.memory.bios_value = 0xE25E_F004,
            Exception::Swi => self.memory.bios_value = 0xE3A0_2004,
            _ => (),
        }
    }

    fn pipeline_stalled(&mut self) {
        self.memory.prefetch_len = 0;
    }

    fn will_execute(&mut self, pc: u32) {
        if self.apu.hle_hook == pc {
            MusicPlayer::pc_match(self);
        }
    }

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        GameGirlAdv::get(self, addr)
    }

    fn set<T: RwType>(&mut self, addr: u32, value: T) {
        match T::WIDTH {
            1 => self.set_byte(addr, value.u8()),
            2 => self.set_hword(addr, value.u16()),
            _ => self.set_word(addr, value.u32()),
        }
    }

    fn wait_time<T: RwType>(&mut self, addr: u32, access: Access) -> u16 {
        self.wait_time::<T>(addr, access)
    }

    fn debugger(&mut self) -> &mut Debugger<u32> {
        &mut self.debugger
    }

    fn can_cache_at(pc: u32) -> bool {
        pc < 0x3FFF
            || (0x300_0000..=(0x300_7FFF - 0x400)).contains(&pc)
            || (0x800_0000..=0xDFF_FFFF).contains(&pc)
    }
}
