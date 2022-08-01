// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use bitmatch::bitmatch;

use super::interface::{ArmSystem, SysWrapper};
use crate::{components::arm::Cpu, numutil::NumExt};

/// Macro for creating accessors for mode-dependent registers.
macro_rules! mode_reg {
    ($reg:ident, $get:ident, $set:ident) => {
        pub fn $get(&self) -> u32 {
            let mode = self.mode();
            if mode == Mode::System {
                self.$reg[0]
            } else {
                self.$reg[mode as usize]
            }
        }

        pub fn $set(&mut self, val: u32) {
            let mode = self.mode();
            if mode == Mode::System {
                self.$reg[0] = val;
            } else {
                self.$reg[mode as usize] = val;
            }
        }
    };
}

/// A register with values for FIQ and all other modes
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FiqReg {
    pub reg: u32,
    pub fiq: u32,
}

/// A register with different values for the different CPU modes
pub type ModeReg = [u32; 6];

/// Execution context of the CPU.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Mode {
    User,
    Fiq,
    Supervisor,
    Abort,
    Irq,
    Undefined,
    System,
}

impl Mode {
    #[bitmatch]
    pub fn get(n: u32) -> Self {
        #[bitmatch]
        match n {
            "0??00" => Self::User,
            "0??01" => Self::Fiq,
            "0??10" => Self::Irq,
            "0??11" => Self::Supervisor,
            "10000" => Self::User,
            "10001" => Self::Fiq,
            "10010" => Self::Irq,
            "10011" => Self::Supervisor,
            "10111" => Self::Abort,
            "11011" => Self::Undefined,
            "11111" => Self::System,
            _ => panic!(),
        }
    }

    pub fn to_u32(self) -> u32 {
        match self {
            Self::User => 0b10000,
            Self::Fiq => 0b10001,
            Self::Irq => 0b10010,
            Self::Supervisor => 0b10011,
            Self::Abort => 0b10111,
            Self::Undefined => 0b11011,
            Self::System => 0b11111,
        }
    }
}

/// Flags inside CPSR.
pub enum Flag {
    Neg = 31,
    Zero = 30,
    Carry = 29,
    Overflow = 28,
    QClamped = 27,
    IrqDisable = 7,
    FiqDisable = 6,
    Thumb = 5,
}

impl Flag {
    pub fn mask(self) -> u16 {
        1 << self as u16
    }
}

impl<S: ArmSystem> Cpu<S> {
    #[inline]
    pub fn sp(&self) -> u32 {
        self.registers[13]
    }

    #[inline]
    pub fn lr(&self) -> u32 {
        self.registers[14]
    }

    #[inline]
    pub fn pc(&self) -> u32 {
        self.registers[15]
    }

    #[inline]
    pub fn set_sp(&mut self, value: u32) {
        self.registers[13] = value;
    }

    #[inline]
    pub fn set_lr(&mut self, value: u32) {
        self.registers[14] = value;
    }

    /// Get the current CPU mode.
    pub fn mode(&self) -> Mode {
        Mode::get(self.cpsr & 0x1F)
    }

    /// Set the mode bits inside CPSR.
    pub fn set_mode(&mut self, ctx: Mode) {
        self.set_cpsr((self.cpsr & !0x1F) | ctx.to_u32());
    }

    #[inline]
    pub fn flag(&self, flag: Flag) -> bool {
        self.cpsr.is_bit(flag as u16)
    }

    #[inline]
    pub fn set_flag(&mut self, flag: Flag, en: bool) {
        self.cpsr = self.cpsr.set_bit(flag as u16, en);
    }

    /// Get the 'adjusted' value of the PC that some instructions need.
    #[inline]
    pub fn adj_pc(&self) -> u32 {
        self.registers[15] & !2
    }

    mode_reg!(sp, cpsr_sp, set_cpsr_sp);
    mode_reg!(lr, cpsr_lr, set_cpsr_lr);
    mode_reg!(spsr, spsr, set_spsr);

    #[inline]
    pub fn low(&self, idx: u16) -> u32 {
        self.registers[idx.us()]
    }

    pub fn reg(&self, idx: u32) -> u32 {
        self.registers[idx.us()]
    }

    pub fn reg_pc4(&self, idx: u32) -> u32 {
        let mut regs = self.registers;
        regs[15] += 4;
        regs[idx.us()]
    }

    fn get_cpsr_reg(&self, idx: u32) -> u32 {
        match idx {
            8..=12 if self.mode() == Mode::Fiq => self.fiqs[(idx - 8).us()].fiq,
            8..=12 => self.fiqs[(idx - 8).us()].reg,
            13 => self.cpsr_sp(),
            14 => self.cpsr_lr(),
            _ => panic!("invalid reg"),
        }
    }

    fn set_cpsr_reg(&mut self, idx: u32, val: u32) {
        match idx {
            8..=12 if self.mode() == Mode::Fiq => self.fiqs[(idx - 8).us()].fiq = val,
            8..=12 => self.fiqs[(idx - 8).us()].reg = val,
            13 => self.set_cpsr_sp(val),
            14 => self.set_cpsr_lr(val),
            _ => panic!("invalid reg"),
        }
    }

    pub fn set_cpsr(&mut self, value: u32) {
        for reg in 8..15 {
            self.set_cpsr_reg(reg, self.registers[reg.us()]);
        }
        self.cpsr = value;
        for reg in 8..15 {
            self.registers[reg.us()] = self.get_cpsr_reg(reg);
        }
    }
}

impl<S: ArmSystem> SysWrapper<S> {
    /// Set the PC. Needs special behavior to fake the pipeline.
    #[inline]
    pub fn set_pc(&mut self, val: u32) {
        // Align to 2/4 depending on mode
        self.cpu().registers[15] = val & (!(self.cpu().inst_size() - 1));
        Cpu::pipeline_stall(&mut **self);
    }

    pub fn set_reg(&mut self, idx: u32, val: u32) {
        if idx == 15 {
            self.set_pc(val);
        } else {
            self.cpu().registers[idx.us()] = val;
        }
    }

    pub fn reg(&self, idx: u32) -> u32 {
        self.cpur().reg(idx)
    }

    pub fn reg_pc4(&self, idx: u32) -> u32 {
        self.cpur().reg_pc4(idx)
    }

    pub fn low(&self, idx: u16) -> u32 {
        self.cpur().low(idx)
    }
}
