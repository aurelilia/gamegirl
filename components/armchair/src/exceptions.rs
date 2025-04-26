// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::NumExt;

use crate::{
    interface::Bus,
    memory::Address,
    state::{
        CpuState,
        Flag::{FiqDisable, IrqDisable, Thumb},
        Mode,
    },
    Cpu,
};

impl CpuState {
    /// An exception occurred, jump to the bootrom handler and deal with it.
    pub(crate) fn exception_occured<S: Bus>(&mut self, bus: &mut S, kind: Exception) {
        bus.exception_happened(self, kind);
        if self.is_flag(Thumb) {
            self.bump_pc(2); // ??
        }

        let cpsr = self.cpsr();
        self.set_mode(kind.mode());

        self.set_flag(Thumb, false);
        self.set_flag(IrqDisable, true);
        if let Exception::Reset | Exception::Fiq = kind {
            self.set_flag(FiqDisable, true);
        }

        let lr = self.pc() - Address(self.current_instruction_size());
        self.set_lr(lr);
        self.set_spsr(cpsr);
        self.set_pc(bus, S::CONFIG.exception_vector_base_address + kind.vector());
    }

    /// Request an interrupt. Will check if the CPU will service it right away.
    pub fn request_interrupt(&mut self, bus: &mut impl Bus, int: Interrupt) {
        self.request_interrupt_with_index(bus, int as u16);
    }

    /// Request an interrupt by index. Will check if the CPU will service it
    /// right away.
    pub fn request_interrupt_with_index(&mut self, bus: &mut impl Bus, idx: u16) {
        self.intr.if_.set_bit(idx, true);
        self.check_if_interrupt(bus);
    }

    fn is_interrupt_pending(&self) -> bool {
        self.intr.ime && !self.is_flag(IrqDisable) && (self.intr.ie & self.intr.if_) != 0
    }

    /// Check if an interrupt needs to be handled and jump to the handler if so.
    /// Called on any events that might cause an interrupt to be triggered.
    pub fn check_if_interrupt(&mut self, bus: &mut impl Bus) {
        if self.is_interrupt_pending() {
            self.bump_pc(4);
            self.exception_occured(bus, Exception::Irq);
        }
    }

    /// Immediately halt the CPU until an IRQ is pending
    pub fn halt_on_irq(&mut self) {
        self.is_halted = true;
    }
}

impl<S: Bus> Cpu<S> {
    /// Request an interrupt. Will check if the CPU will service it right away.
    pub fn request_interrupt(&mut self, int: Interrupt) {
        self.state.request_interrupt(&mut self.bus, int);
    }

    /// Request an interrupt by index. Will check if the CPU will service it
    /// right away.
    pub fn request_interrupt_with_index(&mut self, idx: u16) {
        self.state.request_interrupt_with_index(&mut self.bus, idx);
    }

    pub(crate) fn exception_occured(&mut self, kind: Exception) {
        self.state.exception_occured(&mut self.bus, kind);
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

#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InterruptController {
    pub ime: bool,
    pub ie: u32,
    pub if_: u32,
}
