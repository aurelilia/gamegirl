use bitmatch::bitmatch;
use serde::{Deserialize, Serialize};

use crate::{
    gga::{cpu::Cpu, GameGirlAdv},
    numutil::NumExt,
};

/// Macro for creating accessors for mode-dependent registers.
macro_rules! mode_reg {
    ($reg:ident, $set:ident) => {
        pub fn $reg(&self) -> u32 {
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
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
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
    IrqDisable = 7,
    FiqDisable = 6,
    Thumb = 5,
}

impl Cpu {
    /// Get the current CPU mode.
    pub fn mode(&self) -> Mode {
        Mode::get(self.cpsr & 0x1F)
    }

    /// Set the mode bits inside CPSR.
    pub fn set_mode(&mut self, ctx: Mode) {
        self.cpsr = (self.cpsr & !0x1F) | ctx.to_u32()
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
        self.pc & !2
    }

    mode_reg!(sp, set_sp);
    mode_reg!(lr, set_lr);
    mode_reg!(spsr, set_spsr);

    #[inline]
    pub fn low(&self, idx: u16) -> u32 {
        self.low[idx.us()]
    }

    #[inline]
    pub fn set_low(&mut self, inst: u32, pos: u32, val: u32) {
        self.low[((inst >> pos) & 7) as usize] = val;
    }

    pub fn reg(&self, idx: u32) -> u32 {
        match idx {
            0..=7 => self.low[idx.us()],
            8..=12 if self.mode() == Mode::Fiq => self.fiqs[(idx - 8).us()].fiq,
            8..=12 => self.fiqs[(idx - 8).us()].reg,
            13 => self.sp(),
            14 => self.lr(),
            _ => self.pc,
        }
    }

    pub fn reg_pc4(&self, idx: u32) -> u32 {
        match idx {
            0..=7 => self.low[idx.us()],
            8..=12 if self.mode() == Mode::Fiq => self.fiqs[(idx - 8).us()].fiq,
            8..=12 => self.fiqs[(idx - 8).us()].reg,
            13 => self.sp(),
            14 => self.lr(),
            _ => self.pc + 4,
        }
    }
}

impl GameGirlAdv {
    /// Set the PC. Needs special behavior to fake the pipeline.
    #[inline]
    pub fn set_pc(&mut self, val: u32) {
        // Align to 2/4 depending on mode
        self.cpu.pc = val & (!(self.cpu.inst_size() - 1));
        Cpu::pipeline_stall(self);
    }

    pub fn set_reg(&mut self, idx: u32, val: u32) {
        match idx {
            0..=7 => self.cpu.low[idx.us()] = val,
            8..=12 if self.cpu.mode() == Mode::Fiq => self.cpu.fiqs[(idx - 8).us()].fiq = val,
            8..=12 => self.cpu.fiqs[(idx - 8).us()].reg = val,
            13 => self.cpu.set_sp(val),
            14 => self.cpu.set_lr(val),
            _ => self.set_pc(val),
        }
    }
}
