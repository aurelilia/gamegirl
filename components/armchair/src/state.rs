// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use core::{
    fmt::Display,
    ops::{Index, IndexMut, Range},
};

use bitmatch::bitmatch;
use common::numutil::NumExt;

use crate::{
    interface::{Bus, CpuVersion},
    memory::{
        access::{CODE, NONSEQ, SEQ},
        Access, Address,
    },
    Cpu, InterruptController,
};

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

        pub(crate) fn $set(&mut self, val: u32) {
            let mode = self.mode();
            if mode == Mode::System {
                self.$reg[0] = val;
            } else {
                self.$reg[mode as usize] = val;
            }
        }
    };
}

#[derive(Copy, Clone, PartialEq)]
pub struct LowRegister(pub u16);

impl LowRegister {
    pub fn all() -> impl DoubleEndedIterator<Item = LowRegister> {
        Self::range(0..8)
    }

    pub fn from_rlist(rlist: u8) -> impl DoubleEndedIterator<Item = LowRegister> {
        Self::all().filter(move |r| rlist.is_bit(r.0))
    }

    pub fn range(range: Range<u16>) -> impl DoubleEndedIterator<Item = LowRegister> {
        range.into_iter().map(Self)
    }
}

impl Display for LowRegister {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "r{}", self.0)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct Register(pub u16);

impl Register {
    pub fn is_pc(&self) -> bool {
        self.0 == 15
    }

    pub fn from_rlist(rlist: u16) -> impl DoubleEndedIterator<Item = Register> {
        (0..16)
            .into_iter()
            .map(Self)
            .filter(move |r| rlist.is_bit(r.0))
    }
}

impl Display for Register {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            15 => write!(f, "pc"),
            14 => write!(f, "lr"),
            13 => write!(f, "sp"),
            r => write!(f, "r{r}"),
        }
    }
}

/// A register with values for FIQ and all other modes
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FiqReg {
    pub reg: u32,
    pub fiq: u32,
}

/// A register with different values for the different CPU modes
type ModeReg = [u32; 6];

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CpuState {
    // Registers
    pub registers: [u32; 16],
    fiqs: [FiqReg; 5],
    pub sp: ModeReg,
    lr: ModeReg,
    cpsr: u32,
    spsr: ModeReg,

    // Pipeline + Memory
    pipeline: [u32; 2],
    pipeline_valid: bool,
    pub access_type: Access,

    // Interrupt control
    pub intr: InterruptController,
    pub is_halted: bool,
}

impl CpuState {
    #[inline]
    pub fn sp(&self) -> Address {
        Address(self.registers[13])
    }

    #[inline]
    pub fn lr(&self) -> Address {
        Address(self.registers[14])
    }

    #[inline]
    pub fn pc(&self) -> Address {
        Address(self.registers[15])
    }

    #[inline]
    pub fn cpsr(&self) -> u32 {
        self.cpsr
    }

    #[inline]
    pub fn set_sp(&mut self, value: Address) {
        self.registers[13] = value.0;
    }

    #[inline]
    pub fn set_lr(&mut self, value: Address) {
        self.registers[14] = value.0;
    }

    /// Get the 'adjusted' value of the PC that some instructions need.
    #[inline]
    pub(crate) fn adj_pc(&self) -> Address {
        Address(self.registers[15] & !2)
    }

    #[inline]
    pub(crate) fn bump_pc(&mut self, count: u32) -> Address {
        self.registers[15] = self.registers[15].wrapping_add(count);
        Address(self.registers[15])
    }

    mode_reg!(sp, cpsr_sp, set_cpsr_sp);
    mode_reg!(lr, cpsr_lr, set_cpsr_lr);
    mode_reg!(spsr, spsr, set_spsr);

    /// Get a register's value for the next instruction (PC will be +4)
    pub(crate) fn reg_pc4(&self, reg: Register) -> u32 {
        let mut regs = self.registers;
        regs[15] += 4;
        regs[reg.0.us()]
    }

    /// Set the PC. Needs special behavior to fake the pipeline.
    pub(crate) fn set_pc(&mut self, bus: &mut impl Bus, val: Address) {
        // Align to 2/4 depending on mode
        self.registers[15] = val.0 & (!(self.current_instruction_size() - 1));
        self.pipeline_stall(bus);
    }

    #[inline]
    pub fn is_flag(&self, flag: Flag) -> bool {
        self.cpsr.is_bit(flag as u16)
    }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: Flag, en: bool) {
        self.cpsr = self.cpsr.set_bit(flag as u16, en);
    }

    /// Get the current CPU mode.
    pub fn mode(&self) -> Mode {
        Mode::get(self.cpsr & 0x1F)
    }

    /// Set the mode bits inside CPSR.
    pub(crate) fn set_mode(&mut self, ctx: Mode) {
        self.set_cpsr((self.cpsr & !0x1F) | ctx.to_u32());
    }

    /// Set the CPSR. This may only change flags; mode changes will not be
    /// handled.
    pub(crate) fn set_cpsr_flags(&mut self, value: u32) {
        self.cpsr = value;
    }

    /// Set the CPSR. Needs to consider mode switches, in which case
    /// the current registers need to be copied.
    pub fn set_cpsr(&mut self, value: u32) {
        for reg in 8..=12 {
            if self.mode() == Mode::Fiq {
                self.fiqs[reg - 8].fiq = self.registers[reg];
            } else {
                self.fiqs[reg - 8].reg = self.registers[reg];
            }
        }
        self.set_cpsr_sp(self.registers[13]);
        self.set_cpsr_lr(self.registers[14]);

        self.cpsr = value;

        for reg in 8..=12 {
            self.registers[reg] = if self.mode() == Mode::Fiq {
                self.fiqs[reg - 8].fiq
            } else {
                self.fiqs[reg - 8].reg
            };
        }
        self.registers[13] = self.cpsr_sp();
        self.registers[14] = self.cpsr_lr();
    }

    /// Evaluate a condition encoded into an instruction.
    pub(crate) fn eval_condition(&self, cond: u16) -> bool {
        // This condition table is taken from mGBA sources, which are licensed under
        // MPL2 at https://github.com/mgba-emu/mgba
        // Thank you to endrift and other mGBA contributors!
        const COND_MASKS: [u16; 16] = [
            0xF0F0, // EQ [-Z--]
            0x0F0F, // NE [-z--]
            0xCCCC, // CS [--C-]
            0x3333, // CC [--c-]
            0xFF00, // MI [N---]
            0x00FF, // PL [n---]
            0xAAAA, // VS [---V]
            0x5555, // VC [---v]
            0x0C0C, // HI [-zC-]
            0xF3F3, // LS [-Z--] || [--c-]
            0xAA55, // GE [N--V] || [n--v]
            0x55AA, // LT [N--v] || [n--V]
            0x0A05, // GT [Nz-V] || [nz-v]
            0xF5FA, // LE [-Z--] || [Nz-v] || [nz-V]
            0xFFFF, // AL [----]
            0x0000, // NV
        ];

        let flags = self.cpsr >> 28;
        (COND_MASKS[cond.us()] & (1 << flags)) != 0
    }

    pub fn current_instruction_size(&self) -> u32 {
        // 4 on ARM, 2 on THUMB
        4 - ((self.is_flag(Flag::Thumb) as u32) << 1)
    }

    pub fn next_instruction_offset(&self) -> Address {
        Address(self.current_instruction_size())
    }
}

impl CpuState {
    pub(crate) fn fill_pipeline(&mut self, with: [u32; 2]) {
        self.pipeline = with;
        self.pipeline_valid = true;
    }

    pub(crate) fn invalidate_pipeline(&mut self) {
        self.pipeline_valid = false;
    }

    pub(crate) fn advance_pipeline(&mut self, next: u32) -> u32 {
        let inst = self.pipeline[0];
        self.pipeline[0] = self.pipeline[1];
        self.pipeline[1] = next;
        self.access_type = SEQ;
        inst
    }

    /// Update the pipeline to be valid again, without wait states or actual
    /// reads
    pub(crate) fn revalidate_pipeline(&mut self, bus: &mut impl Bus) {
        if self.pipeline_valid {
            return;
        }
        let p = if self.is_flag(Flag::Thumb) {
            [
                bus.get::<u16>(self, self.pc() - Address::HW).u32(),
                bus.get::<u16>(self, self.pc()).u32(),
            ]
        } else {
            [
                bus.get::<u32>(self, self.pc() - Address::WORD),
                bus.get::<u32>(self, self.pc()),
            ]
        };
        self.fill_pipeline(p);
    }

    /// Emulate a pipeline stall / fill; used when PC changes.
    pub(crate) fn pipeline_stall(&mut self, bus: &mut impl Bus) {
        bus.pipeline_stalled(self);
        if self.is_flag(Flag::Thumb) {
            let time = bus.wait_time::<u16>(self, self.pc(), NONSEQ | CODE);
            bus.tick(time as u64);
            self.bump_pc(2);
            let time = bus.wait_time::<u16>(self, self.pc(), SEQ | CODE);
            bus.tick(time as u64);
        } else {
            let time = bus.wait_time::<u32>(self, self.pc(), NONSEQ | CODE);
            bus.tick(time as u64);
            self.bump_pc(4);
            let time = bus.wait_time::<u32>(self, self.pc(), SEQ | CODE);
            bus.tick(time as u64);
        };
        self.invalidate_pipeline();
        self.access_type = SEQ;
    }
}

impl Default for CpuState {
    fn default() -> Self {
        Self {
            registers: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4],
            fiqs: Default::default(),
            sp: [0x0300_7F00, 0x0, 0x0300_7FE0, 0x0, 0x0300_7FA0, 0x0],
            lr: Default::default(),
            cpsr: 0xD3,
            spsr: Default::default(),
            pipeline: Default::default(),
            pipeline_valid: Default::default(),
            access_type: Default::default(),
            intr: Default::default(),
            is_halted: Default::default(),
        }
    }
}

impl<S: Bus> Cpu<S> {
    pub(crate) fn set_pc(&mut self, val: Address) {
        self.state.set_pc(&mut self.bus, val);
    }
}

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

impl<S: Bus> Cpu<S> {
    /// Set a register. Needs special behavior due to PC.
    pub(crate) fn set_reg(&mut self, reg: Register, val: u32) {
        if reg.is_pc() {
            self.state.set_pc(&mut self.bus, Address(val));
        } else {
            self.state.registers[reg.0.us()] = val;
        }
    }

    /// Set a register. Needs special behavior due to PC.
    /// Additionally allows a mode switch when setting PC.
    pub(crate) fn set_reg_allow_switch(&mut self, reg: Register, val: u32) {
        if reg.is_pc() {
            if S::Version::IS_V5 {
                self.state.set_flag(Flag::Thumb, val.is_bit(0));
            }
            self.state.set_pc(&mut self.bus, Address(val));
        } else {
            self.state.registers[reg.0.us()] = val;
        }
    }

    /// Get a register's value for the next instruction (PC will be +4)
    pub(crate) fn reg_pc4(&self, reg: Register) -> u32 {
        self.state.reg_pc4(reg)
    }
}

impl Index<LowRegister> for CpuState {
    type Output = u32;

    fn index(&self, index: LowRegister) -> &Self::Output {
        &self.registers[index.0.us()]
    }
}

impl IndexMut<LowRegister> for CpuState {
    fn index_mut(&mut self, index: LowRegister) -> &mut Self::Output {
        &mut self.registers[index.0.us()]
    }
}

impl Index<Register> for CpuState {
    type Output = u32;

    fn index(&self, index: Register) -> &Self::Output {
        &self.registers[index.0.us()]
    }
}
