// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::{
    ggc::{
        io::addr::{IE, IF},
        GameGirl,
    },
    numutil::NumExt,
};

mod alu;
mod data;
pub mod inst;

/// The system CPU and it's registers.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Cpu {
    pub pc: u16,
    pub sp: u16,
    pub ime: bool,
    halt_bug: bool,
    regs: [u8; 8],
}

impl Cpu {
    /// Execute the next instruction, moving the entire system forward.
    pub(super) fn exec_next_inst(gg: &mut GameGirl) {
        if !gg.debugger.should_execute(gg.cpu.pc) {
            gg.options.running = false; // Pause emulation, we hit a BP
            return;
        }
        let ime = gg.cpu.ime;

        let inst = inst::get_next(gg);
        if gg.cpu.halt_bug {
            gg.cpu.pc -= 1;
            gg.cpu.halt_bug = false;
        }

        let inc = inst::execute(gg, inst);
        if inc {
            gg.cpu.pc += inst.size().u16();
        }

        Self::check_interrupts(gg, ime && gg.cpu.ime);
    }

    /// Check if any interrupts occurred.
    fn check_interrupts(gg: &mut GameGirl, ime: bool) {
        let mut bits = gg[IE] & gg[IF] & 0x1F;
        if !ime || (bits == 0) {
            return;
        }

        for bit in 0..5 {
            if bits.is_bit(bit) {
                if Self::dispatch_intr(gg, bit) {
                    return;
                } else {
                    bits = gg[IE] & gg[IF] & 0x1F;
                }
            }
        }
    }

    fn dispatch_intr(gg: &mut GameGirl, intr: u16) -> bool {
        gg.cpu.ime = false;
        // If the PC push will overwrite IE with the high byte.
        let upper_ie_push = gg.cpu.sp == 0;
        gg.push_stack(gg.cpu.pc);

        if !upper_ie_push || gg[IE].is_bit(intr) {
            gg[IF] = gg[IF].set_bit(intr, false) as u8;
            gg.cpu.pc = Interrupt::from_index(intr).addr();
            gg.advance_clock(3);
            true
        } else {
            // Edge case: If the PC push overwrote IE on the high byte, jump to 0 instead
            gg.cpu.pc = 0;
            false
        }
    }

    pub fn flag(&self, flag: Flag) -> bool {
        self.reg(Reg::F).is_bit(flag.bit())
    }

    pub fn set_fli(&mut self, flag: Flag, val: u8) {
        self.set_fl(flag, val != 0)
    }

    pub fn set_fli16(&mut self, flag: Flag, val: u16) {
        self.set_fl(flag, val != 0)
    }

    pub fn set_fl(&mut self, flag: Flag, val: bool) {
        self.regs[Reg::F.i()] = self.reg(Reg::F).set_bit(flag.bit(), val).u8()
    }

    pub fn reg(&self, reg: Reg) -> u8 {
        self.regs[reg.i()]
    }

    pub fn set_reg(&mut self, reg: Reg, value: u8) {
        // Register F only allows writing the 4 high/flag bits
        let value = if reg == Reg::F { value & 0xF0 } else { value };
        self.regs[reg.i()] = value
    }

    pub fn dreg(&self, reg: DReg) -> u16 {
        let low = self.reg(reg.low());
        let high = self.reg(reg.high());
        (high.u16() << 8) | low.u16()
    }

    fn set_dreg(&mut self, reg: DReg, value: u16) {
        self.set_reg(reg.low(), value.u8());
        self.set_reg(reg.high(), (value >> 8).u8());
    }
}

/// The CPU's registers.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Reg {
    A,
    B,
    C,
    D,
    E,
    F,
    H,
    L,
}

impl Reg {
    pub fn i(self) -> usize {
        self as usize
    }
}

/// The CPU's double registers.
#[derive(Debug, Copy, Clone)]
pub enum DReg {
    BC,
    DE,
    HL,
    AF,
}

impl DReg {
    pub fn low(self) -> Reg {
        match self {
            DReg::BC => Reg::C,
            DReg::DE => Reg::E,
            DReg::HL => Reg::L,
            DReg::AF => Reg::F,
        }
    }

    pub fn high(self) -> Reg {
        match self {
            DReg::BC => Reg::B,
            DReg::DE => Reg::D,
            DReg::HL => Reg::H,
            DReg::AF => Reg::A,
        }
    }
}

/// Flags stored in the F register.
#[derive(Copy, Clone)]
pub enum Flag {
    Zero = 7,
    Negative = 6,
    HalfCarry = 5,
    Carry = 4,
}

impl Flag {
    pub fn bit(self) -> u16 {
        self as u16
    }

    pub fn mask(self) -> u8 {
        1 << self as u8
    }

    pub fn from(self, value: u8) -> u8 {
        if value != 0 {
            self.mask().u8()
        } else {
            0
        }
    }
}

/// Interrupts and their vectors.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Interrupt {
    VBlank = 0x0040,
    Stat = 0x0048,
    Timer = 0x0050,
    Serial = 0x0058,
    Joypad = 0x0060,
}

impl Interrupt {
    const ORDER: [Interrupt; 5] = [
        Self::VBlank,
        Self::Stat,
        Self::Timer,
        Self::Serial,
        Self::Joypad,
    ];

    pub fn to_index(self) -> u16 {
        (self.addr() - 0x40) / 8
    }

    pub fn from_index(index: u16) -> Self {
        Self::ORDER[index as usize]
    }

    pub fn addr(self) -> u16 {
        self as u16
    }
}
