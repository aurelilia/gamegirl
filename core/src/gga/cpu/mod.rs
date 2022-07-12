// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

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
            FiqReg,
            Flag::{FiqDisable, IrqDisable, Thumb},
            Mode, ModeReg,
        },
        Access, GameGirlAdv,
    },
    numutil::NumExt,
};

/// Represents the CPU of the console - an ARM7TDMI.
#[derive(Deserialize, Serialize)]
pub struct Cpu {
    pub fiqs: [FiqReg; 5],
    pub sp: ModeReg,
    pub lr: ModeReg,
    pub cpsr: u32,
    pub spsr: ModeReg,

    registers: [u32; 16],
    pipeline: [u32; 2],
    pub(crate) access_type: Access,
}

impl Cpu {
    /// Execute the next instruction and advance the scheduler.
    pub fn exec_next_inst(gg: &mut GameGirlAdv) {
        if !gg.debugger.should_execute(gg.cpu.pc()) {
            gg.options.running = false; // Pause emulation, we hit a BP
            return;
        }

        gg.advance_clock();
        gg.cpu.inc_pc();

        if gg.cpu.flag(Thumb) {
            let inst = gg.cpu.pipeline[0].u16();
            gg.cpu.pipeline[0] = gg.cpu.pipeline[1];
            gg.cpu.pipeline[1] = gg.read_hword(gg.cpu.pc(), gg.cpu.access_type);
            gg.cpu.access_type = Access::Seq;
            gg.execute_inst_thumb(inst);

            if crate::TRACING {
                let mnem = GameGirlAdv::get_mnemonic_thumb(inst);
                println!("0x{:08X} {}", gg.cpu.pc(), mnem);
            }
        } else {
            let inst = gg.cpu.pipeline[0];
            gg.cpu.pipeline[0] = gg.cpu.pipeline[1];
            gg.cpu.pipeline[1] = gg.read_word(gg.cpu.pc(), gg.cpu.access_type);
            gg.cpu.access_type = Access::Seq;
            gg.execute_inst_arm(inst);

            if crate::TRACING {
                let mnem = GameGirlAdv::get_mnemonic_arm(inst);
                println!("0x{:08X} {}", gg.cpu.pc(), mnem);
            }
        }
    }

    /// Check if an interrupt needs to be handled and jump to the handler if so.
    /// Called on any events that might cause an interrupt to be triggered..
    pub fn check_if_interrupt(gg: &mut GameGirlAdv) {
        let int = (gg[IME] == 1) && !gg.cpu.flag(IrqDisable) && (gg[IE] & gg[IF]) != 0;
        if int {
            gg.cpu.inc_pc_by(4);
            Cpu::exception_occurred(gg, Exception::Irq);
        }
    }

    /// An exception occurred, jump to the bootrom handler and deal with it.
    fn exception_occurred(gg: &mut GameGirlAdv, kind: Exception) {
        if gg.cpu.pc() > 0x100_0000 {
            gg.memory.bios_value = 0xE25EF004;
        }
        if gg.cpu.flag(Thumb) {
            gg.cpu.inc_pc_by(2); // ??
        }

        let cpsr = gg.cpu.cpsr;
        gg.cpu.set_mode(kind.mode());

        gg.cpu.set_flag(Thumb, false);
        gg.cpu.set_flag(IrqDisable, true);
        if let Exception::Reset | Exception::Fiq = kind {
            gg.cpu.set_flag(FiqDisable, true);
        }

        gg.cpu.set_lr(gg.cpu.pc() - gg.cpu.inst_size());
        gg.cpu.set_spsr(cpsr);
        gg.set_pc(kind.vector());
    }

    /// Emulate a pipeline stall / fill; used when PC changes.
    pub fn pipeline_stall(gg: &mut GameGirlAdv) {
        gg.memory.prefetch_len = 0; // Discard prefetch
        if gg.cpu.flag(Thumb) {
            gg.cpu.pipeline[0] = gg.read_hword(gg.cpu.pc(), Access::NonSeq);
            gg.cpu.pipeline[1] = gg.read_hword(gg.cpu.pc() + 2, Access::Seq);
        } else {
            gg.cpu.pipeline[0] = gg.read_word(gg.cpu.pc(), Access::NonSeq);
            gg.cpu.pipeline[1] = gg.read_word(gg.cpu.pc() + 4, Access::Seq);
        };
        gg.cpu.access_type = Access::Seq;
        gg.cpu.inc_pc();
    }

    #[inline]
    fn inc_pc(&mut self) {
        self.inc_pc_by(self.inst_size());
    }

    #[inline]
    fn inc_pc_by(&mut self, count: u32) {
        self.registers[15] = self.registers[15].wrapping_add(count);
    }

    #[inline]
    pub fn inst_size(&self) -> u32 {
        // 4 on ARM, 2 on THUMB
        4 - ((self.flag(Thumb) as u32) << 1)
    }

    /// Request an interrupt. Will check if the CPU will service it right away.
    #[inline]
    pub fn request_interrupt(gg: &mut GameGirlAdv, int: Interrupt) {
        Self::request_interrupt_idx(gg, int as u16);
    }

    /// Request an interrupt by index. Will check if the CPU will service it
    /// right away.
    #[inline]
    pub fn request_interrupt_idx(gg: &mut GameGirlAdv, idx: u16) {
        gg[IF] = gg[IF].set_bit(idx, true);
        Self::check_if_interrupt(gg);
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            fiqs: [FiqReg::default(); 5],
            sp: [0x0300_7F00, 0x0, 0x0300_7FE0, 0x0, 0x0300_7FA0, 0x0],
            lr: ModeReg::default(),
            cpsr: 0xD3,
            spsr: ModeReg::default(),
            registers: [0; 16],
            pipeline: [0; 2],
            access_type: Access::NonSeq,
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
/// Most are only listed to preserve bit order in IE/IF, only SWI
/// and IRQ ever get raised on the GGA. (UND does as well, but this
/// emulator doesn't implement that.)
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
    /// Vector to set the PC to when this exception occurs.
    fn vector(self) -> u32 {
        self as u32 * 4
    }

    /// Mode to execute the exception in.
    fn mode(self) -> Mode {
        const MODE: [Mode; 8] = [
            Mode::Supervisor,
            Mode::Undefined,
            Mode::Supervisor,
            Mode::Abort,
            Mode::Abort,
            Mode::Supervisor,
            Mode::Irq,
            Mode::Fiq,
        ];
        MODE[self as usize]
    }
}
