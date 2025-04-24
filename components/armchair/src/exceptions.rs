// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::format;

use common::{common::debugger::Severity, numutil::NumExt};

use crate::{
    interface::Bus,
    memory::Address,
    registers::{
        Flag::{FiqDisable, IrqDisable, Thumb},
        Mode,
    },
    Cpu,
};

impl<S: Bus> Cpu<S> {
    /// Check if an interrupt needs to be handled and jump to the handler if so.
    /// Called on any events that might cause an interrupt to be triggered..
    pub fn check_if_interrupt(&mut self) {
        if self.is_interrupt_pending() {
            self.regs.bump_pc(4);
            self.exception_occured(Exception::Irq);
        }
    }

    fn is_interrupt_pending(&self) -> bool {
        self.ime && !self.regs.is_flag(IrqDisable) && (self.ie & self.if_) != 0
    }

    /// An exception occurred, jump to the bootrom handler and deal with it.
    pub(crate) fn exception_occured(&mut self, kind: Exception) {
        self.bus.exception_happened(kind);
        if self.regs.is_flag(Thumb) {
            self.regs.bump_pc(2); // ??
        }

        let cpsr = self.regs.cpsr();
        self.regs.set_mode(kind.mode());

        self.regs.set_flag(Thumb, false);
        self.regs.set_flag(IrqDisable, true);
        if let Exception::Reset | Exception::Fiq = kind {
            self.debugger.log(
                "exception-raised",
                format!("An unusual exception got raised: {kind:?}"),
                Severity::Warning,
            );
            self.regs.set_flag(FiqDisable, true);
        }

        let lr = self.regs.pc() - Address(self.current_instruction_size());
        self.regs.set_lr(lr);
        self.regs.set_spsr(cpsr);
        self.set_pc(S::CONFIG.exception_vector_base_address + kind.vector());
    }

    /// Request an interrupt. Will check if the CPU will service it right away.
    #[inline]
    pub fn request_interrupt(&mut self, int: Interrupt) {
        self.request_interrupt_with_index(int as u16);
    }

    /// Request an interrupt by index. Will check if the CPU will service it
    /// right away.
    #[inline]
    pub fn request_interrupt_with_index(&mut self, idx: u16) {
        self.if_.set_bit(idx, true);
        self.check_if_interrupt();
    }
}

/// Possible interrupts.
/// These are the same between GGA and NDS, so
/// putting them here is OK.
#[repr(C)]
#[derive(Copy, Clone)]
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
    Unused1,
    Unused2,
    IpcSync,
    IpcSendFifoEmpty,
    IpcRecvFifoNotEmpty,
    CardTransferComplete,
    CardIreqMc,
    GeometryFifo,
    ScreensOpen,
    SpiBus,
    Wifi,
}

/// Possible exceptions.
/// Most are only listed to preserve bit order in IE/IF, only SWI, UND
/// and IRQ ever get raised on the GGA.
#[derive(Debug, Copy, Clone)]
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
    fn vector(self) -> Address {
        Address(self as u32 * 4)
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
