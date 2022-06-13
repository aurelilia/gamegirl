use crate::gga::cpu::registers::Flag::Thumb;
use crate::gga::cpu::Cpu;
use crate::numutil::NumExt;
use bitmatch::bitmatch;
use serde::{Deserialize, Serialize};

/// Macro for creating accessors for mode-dependent registers.
macro_rules! mode_reg {
    ($reg:ident, $set:ident) => {
        pub fn $reg(&self) -> u32 {
            let ctx = self.context();
            if ctx == Context::System {
                self.$reg[0]
            } else {
                self.$reg[self.context() as usize]
            }
        }

        pub fn $set(&mut self, val: u32) {
            let ctx = self.context();
            if ctx == Context::System {
                self.$reg[0] = val;
            } else {
                self.$reg[self.context() as usize] = val;
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

/// CPU state.
pub enum Mode {
    Arm,
    Thumb,
}

/// Execution context of the CPU.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Context {
    User,
    Fiq,
    Supervisor,
    Abort,
    Irq,
    Undefined,
    System,
}

impl Context {
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
    Sign = 31,
    Zero = 30,
    Carry = 29,
    Overflow = 28,
    IrqDisable = 7,
    FiqDisable = 6,
    Thumb = 5,
}

impl Cpu {
    pub fn mode(&self) -> Mode {
        if self.flag(Thumb) {
            Mode::Thumb
        } else {
            Mode::Arm
        }
    }

    pub fn context(&self) -> Context {
        Context::get(self.cpsr & 0x1F)
    }

    pub fn set_context(&mut self, ctx: Context) {
        self.cpsr = (self.cpsr & !0x1F) | ctx.to_u32()
    }

    pub fn flag(&self, flag: Flag) -> bool {
        self.cpsr.is_bit(flag as u16)
    }

    pub fn set_flag(&mut self, flag: Flag, en: bool) {
        self.cpsr = self.cpsr.set_bit(flag as u16, en);
    }

    pub fn adj_pc(&self) -> u32 {
        self.pc & !2
    }

    mode_reg!(sp, set_sp);
    mode_reg!(lr, set_lr);
    mode_reg!(spsr, set_spsr);

    pub fn set_pc(&mut self, val: u32) {
        self.pc_just_changed = true;
        self.pc = val;
    }

    pub fn low(&self, idx: u16) -> u32 {
        self.low[idx.us()]
    }

    pub fn set_low(&mut self, inst: u32, pos: u32, val: u32) {
        self.low[((inst >> pos) & 7) as usize] = val;
    }

    pub fn reg(&self, idx: u32) -> u32 {
        match idx {
            0..=7 => self.low[idx.us()],
            8..=12 if self.context() == Context::Fiq => self.fiqs[(idx - 8).us()].fiq,
            8..=12 => self.fiqs[(idx - 8).us()].reg,
            13 => self.sp(),
            14 => self.lr(),
            _ => self.pc,
        }
    }

    pub fn set_reg(&mut self, idx: u32, val: u32) {
        match idx {
            0..=7 => self.low[idx.us()] = val,
            8..=12 if self.context() == Context::Fiq => self.fiqs[(idx - 8).us()].fiq = val,
            8..=12 => self.fiqs[(idx - 8).us()].reg = val,
            13 => self.set_sp(val),
            14 => self.set_lr(val),
            _ => self.set_pc(val),
        }
    }
}
