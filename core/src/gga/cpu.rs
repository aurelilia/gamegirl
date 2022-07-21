// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    components::arm::{
        inst_arm::ArmLut,
        inst_thumb::ThumbLut,
        interface::{ArmSystem, RwType, SysWrapper},
        Access, Cpu, Exception,
    },
    gga::{addr, addr::WAITCNT, GameGirlAdv},
    numutil::NumExt,
};

pub const CPU_CLOCK: f32 = 2u32.pow(24) as f32;

impl ArmSystem for GameGirlAdv {
    const ARM_LUT: ArmLut<Self> = SysWrapper::<Self>::make_armv4_lut();
    const THUMB_LUT: ThumbLut<Self> = SysWrapper::<Self>::make_thumbv4_lut();
    const IE_ADDR: u32 = addr::IE;
    const IF_ADDR: u32 = addr::IF;
    const IME_ADDR: u32 = addr::IME;

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
        self.scheduler.advance(cycles.u32());
    }

    fn add_i_cycles(&mut self, cycles: u16) {
        self.scheduler.advance(cycles.u32());
        if self.cpu.pc() > 0x800_0000 {
            if self[WAITCNT].is_bit(14) {
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

    fn get<T: RwType>(&mut self, addr: u32) -> T {
        match T::WIDTH {
            1 => T::from_u8(self.get_byte(addr)),
            2 => T::from_u16(self.get_hword(addr)),
            _ => T::from_u32(self.get_word(addr)),
        }
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

    fn check_debugger(&mut self) -> bool {
        self.options.running &= self.debugger.should_execute(self.cpu.pc());
        self.options.running
    }

    fn can_cache_at(pc: u32) -> bool {
        pc < 0x3FFF
            || (0x300_0000..=(0x300_7FFF - 0x400)).contains(&pc)
            || (0x800_0000..=0xDFF_FFFF).contains(&pc)
    }
}
