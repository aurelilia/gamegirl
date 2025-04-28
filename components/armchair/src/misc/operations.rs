use alloc::format;
use core::fmt::UpperHex;

use common::{common::debugger::Severity, numutil::NumExt};

use crate::{
    interface::{Bus, CpuVersion},
    memory::{access::NONSEQ, Access, Address, RelativeOffset},
    state::Register,
    Cpu, Exception,
};

impl<S: Bus> Cpu<S> {
    pub fn und_inst<T: UpperHex>(&mut self, code: T) {
        self.bus.debugger().log(
            "unknown-opcode",
            format!("Unknown opcode '0x{code:X}'"),
            Severity::Error,
        );
        self.exception_occured(Exception::Undefined);
    }

    /// Idle for 1 cycle and set access type to non-sequential.
    pub fn idle_nonseq(&mut self) {
        self.bus.tick(1);
        self.state.access_type = NONSEQ;
    }

    /// Calculate MUL instruction wait cycles for ARMv4 and add them to the
    /// clock.
    pub fn apply_mul_idle_ticks(&mut self, mut value: u32, signed: bool) {
        self.idle_nonseq();
        let mut mask = 0xFFFF_FF00;
        loop {
            value &= mask;
            if value == 0 || (signed && value == mask) {
                break;
            }
            self.bus.tick(1);
            mask <<= 8;
        }
    }

    /// Read a half-word from the bus (LE).
    /// If address is unaligned, do LDRSH behavior.
    pub fn read_hword_ldrsh(&mut self, addr: Address, kind: Access) -> u32 {
        let time = self.bus.wait_time::<u16>(&mut self.state, addr, kind);
        self.bus.tick(time as u64);
        let val = self.bus.get::<u16>(&mut self.state, addr).u32();
        if !S::Version::IS_V5 && addr.0.is_bit(0) {
            // Unaligned on ARMv4
            (val >> 8) as i8 as i32 as u32
        } else {
            // Aligned
            val as i16 as i32 as u32
        }
    }

    /// Read a word from the bus (LE).
    /// If address is unaligned, do LDR/SWP behavior.
    pub fn read_word_ldrswp(&mut self, addr: Address, kind: Access) -> u32 {
        let val = self.read::<u32>(addr, kind);
        if addr.0 & 3 != 0 {
            // Unaligned
            let by = (addr.0 & 3) << 3;
            val.rotate_right(by)
        } else {
            // Aligned
            val
        }
    }

    /// Called by multiple load/store instructions when the Rlist was
    /// empty, which causes R15 to be loaded/stored and Rb to be
    /// incremented/decremented by 0x40.
    pub fn on_empty_rlist(&mut self, rb: Register, str: bool, up: bool, before: bool) {
        let addr = Address(self.state[rb]);
        self.set_reg(rb, (addr.add_signed(Address(0x40), up)).0);

        if !S::Version::IS_V5 && str {
            let addr = match (up, before) {
                (true, true) => addr + Address::WORD,
                (true, false) => addr,
                (false, true) => addr - Address(0x40),
                (false, false) => addr - Address(0x3C),
            };
            let value = self.state.pc().0 + self.state.current_instruction_size();
            self.write::<u32>(addr, value, NONSEQ);
        } else if !S::Version::IS_V5 {
            let val = self.read::<u32>(addr, NONSEQ);
            self.set_pc(Address(val));
        }
    }

    /// Perform a relative jump.
    pub fn relative_jump(&mut self, offset: RelativeOffset) {
        self.state.is_halted = !self
            .opt
            .waitloop
            .on_jump(&self.state, offset, &mut self.opt.table);
        self.set_pc(self.state.pc().add_rel(offset));
    }
}
