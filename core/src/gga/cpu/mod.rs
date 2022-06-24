mod alu;
mod inst_arm;
mod inst_generic;
mod inst_thumb;
pub mod registers;

use serde::{Deserialize, Serialize};

use crate::{
    gga::{
        addr::*,
        cpu::registers::{
            Context, FiqReg,
            Flag::{FiqDisable, IrqDisable, Thumb},
            ModeReg,
        },
        Access, GameGirlAdv,
    },
    numutil::NumExt,
};

pub(crate) const TRACING: bool = false;

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
}

impl Cpu {
    pub fn exec_next_inst(gg: &mut GameGirlAdv) {
        if !gg.debugger.should_execute(gg.cpu.pc) {
            gg.options.running = false; // Pause emulation, we hit a BP
            return;
        }

        gg.advance_clock();
        gg.cpu.inc_pc();

        if gg.cpu.flag(Thumb) {
            let inst = gg.read_hword(gg.cpu.pc - 4, Access::Seq).u16();
            gg.execute_inst_thumb(inst);

            if TRACING {
                let mnem = GameGirlAdv::get_mnemonic_thumb(inst);
                println!("0x{:08X} {}", gg.cpu.pc, mnem);
            }
        } else {
            let inst = gg.read_word(gg.cpu.pc - 8, Access::Seq);
            gg.execute_inst_arm(inst);

            if TRACING {
                let mnem = GameGirlAdv::get_mnemonic_arm(inst);
                println!("0x{:08X} {}", gg.cpu.pc, mnem);
            }
        }
    }

    pub fn check_if_interrupt(gg: &mut GameGirlAdv) {
        let int = (gg[IME] == 1) && !gg.cpu.flag(IrqDisable) && (gg[IE] & gg[IF]) != 0;
        if int {
            gg.cpu.inc_pc_by(4);
            Cpu::exception_occurred(gg, Exception::Irq);
        }
    }

    fn exception_occurred(gg: &mut GameGirlAdv, kind: Exception) {
        if gg.cpu.flag(Thumb) {
            gg.cpu.inc_pc_by(2); // ??
        }

        let cpsr = gg.cpu.cpsr;
        gg.cpu.set_context(kind.context());

        gg.cpu.set_flag(Thumb, false);
        gg.cpu.set_flag(IrqDisable, true);
        if let Exception::Reset | Exception::Fiq = kind {
            gg.cpu.set_flag(FiqDisable, true);
        }

        gg.cpu.set_lr(gg.cpu.pc - gg.cpu.inst_size());
        gg.cpu.set_spsr(cpsr);
        gg.set_pc(kind.vector());
    }

    pub fn pipeline_stall(gg: &mut GameGirlAdv) {
        if gg.cpu.flag(Thumb) {
            gg.add_wait_cycles(gg.wait_time::<2>(gg.cpu.pc, Access::NonSeq));
            gg.add_wait_cycles(gg.wait_time::<2>(gg.cpu.pc + 4, Access::Seq));
        } else {
            gg.add_wait_cycles(gg.wait_time::<4>(gg.cpu.pc, Access::NonSeq));
            gg.add_wait_cycles(gg.wait_time::<4>(gg.cpu.pc + 4, Access::Seq));
        };
        gg.cpu.inc_pc();
    }

    #[inline]
    fn inc_pc(&mut self) {
        self.inc_pc_by(self.inst_size());
    }

    #[inline]
    fn inc_pc_by(&mut self, count: u32) {
        self.pc = self.pc.wrapping_add(count);
    }

    #[inline]
    pub fn inst_size(&self) -> u32 {
        // 4 on ARM, 2 on THUMB
        4 - ((self.flag(Thumb) as u32) << 1)
    }

    #[inline]
    pub fn request_interrupt(gg: &mut GameGirlAdv, int: Interrupt) {
        Self::request_interrupt_idx(gg, int as u16);
    }

    #[inline]
    pub fn request_interrupt_idx(gg: &mut GameGirlAdv, idx: u16) {
        gg[IF] = gg[IF].set_bit(idx, true);
        Self::check_if_interrupt(gg);
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
            cpsr: 0xD3,
            spsr: ModeReg::default(),
        }
    }
}

/// Possible interrupts.
#[repr(C)]
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
