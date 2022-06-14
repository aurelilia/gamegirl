mod alu;
mod inst_arm;
mod inst_thumb;
pub mod registers;

use serde::{Deserialize, Serialize};

use crate::{
    gga::{
        cpu::registers::{
            Context, FiqReg,
            Flag::{FiqDisable, IrqDisable, Thumb},
            ModeReg,
        },
        GameGirlAdv,
    },
    numutil::NumExt,
};
use std::mem;

/// Represents the CPU of the console - an ARM7TDMI.
#[derive(Deserialize, Serialize)]
pub struct Cpu {
    pub low: [u32; 8],
    pub fiqs: [FiqReg; 5],
    pub sp: ModeReg,
    pub lr: ModeReg,
    pub pc: u32,
    pub cpsr: u32,
    pub spsr: ModeReg,

    pub ie: u16,
    pub if_: u16,
    pub ime: bool,
    pub halt: bool,
    pc_just_changed: bool,
    prefetch: [u32; 2],
}

impl Cpu {
    pub fn exec_next_inst(gg: &mut GameGirlAdv) {
        if !gg.debugger.should_execute(gg.cpu.pc) {
            return;
        }

        gg.advance_clock();
        if gg.cpu.halt || gg.cpu.check_interrupt_occurs() {
            return;
        }

        let inst = Self::next_inst(gg);
        if gg.cpu.flag(Thumb) {
            gg.execute_inst_thumb(inst.u16());
        } else {
            gg.execute_inst_arm(inst);
        }

        if gg.cpu.pc_just_changed {
            Self::fix_prefetch(gg);
            gg.cpu.pc_just_changed = false;
        }
    }

    fn next_inst(gg: &mut GameGirlAdv) -> u32 {
        gg.cpu.inc_pc();
        let fetched = Self::inst_at_pc(gg);
        let next = mem::replace(&mut gg.cpu.prefetch[1], fetched);
        mem::replace(&mut gg.cpu.prefetch[0], next)
    }

    fn check_interrupt_occurs(&mut self) -> bool {
        let int = self.ime && !self.flag(IrqDisable) && (self.ie & self.if_) != 0;
        if self.ime && !self.flag(IrqDisable) && (self.ie & self.if_) != 0 {
            self.inc_pc_by(4);
            self.exception_occurred(Exception::Irq);
        }
        int
    }

    fn exception_occurred(&mut self, kind: Exception) {
        self.set_context(kind.context());

        self.set_lr(self.pc - self.inst_size());
        self.set_spsr(self.cpsr);
        self.set_pc(kind.vector());

        self.set_flag(Thumb, false);
        self.set_flag(IrqDisable, true);
        if let Exception::Reset | Exception::Fiq = kind {
            self.set_flag(FiqDisable, true);
        }
    }

    fn inst_at_pc(gg: &mut GameGirlAdv) -> u32 {
        if gg.cpu.flag(Thumb) {
            gg.read_hword(gg.cpu.pc).u32()
        } else {
            gg.read_word(gg.cpu.pc)
        }
    }

    pub fn fix_prefetch(gg: &mut GameGirlAdv) {
        gg.cpu.prefetch[0] = Self::inst_at_pc(gg);
        gg.cpu.inc_pc();
        gg.cpu.prefetch[1] = Self::inst_at_pc(gg);
    }

    fn inc_pc(&mut self) {
        self.inc_pc_by(self.inst_size());
    }

    fn inc_pc_by(&mut self, count: u32) {
        self.pc = self.pc.wrapping_add(count);
    }

    pub fn inst_size(&self) -> u32 {
        // 4 on ARM, 2 on THUMB
        4 - ((self.flag(Thumb) as u32) << 1)
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            low: [0; 8],
            fiqs: [FiqReg::default(); 5],
            sp: [0x0300_7F00, 0x0, 0x0300_7FE0, 0x0, 0x0300_7FA0, 0x0],
            lr: ModeReg::default(),
            pc: 0,
            cpsr: 0x1F,
            spsr: ModeReg::default(),
            ie: 0,
            if_: 0,
            ime: false,
            halt: false,
            pc_just_changed: false,
            prefetch: [0, 0],
        }
    }
}

/// Possible interrupts.
pub enum Interrupt {
    VBlank,
    HBlank,
    VCounter,
    Timer0,
    Timer1,
    Timer2,
    Timer3,
    Serial,
    Dma0,
    Dma1,
    Dma2,
    Dma3,
    Joypad,
    GamePak,
}

/// Possible exceptions.
#[derive(Copy, Clone)]
pub enum Exception {
    Reset,
    Undefined,
    Swi,
    PrefetchAbort,
    DataAbort,
    AddressExceeded,
    Irq,
    Fiq,
}

impl Exception {
    fn vector(self) -> u32 {
        self as u32 * 4
    }

    fn context(self) -> Context {
        const CTX: [Context; 8] = [
            Context::Supervisor,
            Context::Undefined,
            Context::Supervisor,
            Context::Abort,
            Context::Abort,
            Context::Supervisor,
            Context::Irq,
            Context::Fiq,
        ];
        CTX[self as usize]
    }
}
