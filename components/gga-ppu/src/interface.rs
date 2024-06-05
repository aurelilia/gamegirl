// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::ops::IndexMut;

use common::TimeS;

use crate::{
    threading::{GgaPpu, PpuMmio},
    PpuEvent,
};

/// Interface for a system with a GGA PPU.
/// This needs to be implemented once per PPU, so NDS implements it twice.
pub trait PpuSystem: IndexMut<u32, Output = u16> + Sized + 'static {
    /// Width of the display
    const W: usize;
    /// Height of the display; visible scanlines
    const H: usize;
    /// End of VBlank scanline; height of the screen + VBlank scanline count
    const VBLANK_END: usize;

    /// Get the PPU.
    fn ppu(&mut self) -> &mut GgaPpu<Self>
    where
        [(); Self::W * Self::H]:;
    /// Get the PPU.
    fn ppur(&self) -> &GgaPpu<Self>
    where
        [(); Self::W * Self::H]:;
    /// Get the PPU IO registers for rendering a scanline.
    fn ppu_mmio(&mut self) -> PpuMmio;

    /// Request an interrupt.
    fn request_interrupt(&mut self, int: PpuInterrupt);
    /// Notify the DMAs about a certain event.
    fn notify_dma(&mut self, reason: PpuDmaReason);
    /// Schedule an event on the scheduler,
    fn schedule(&mut self, evt: PpuEvent, at: TimeS);
}

/// Interrupts the PPU can raise.
#[repr(C)]
pub enum PpuInterrupt {
    VBlank,
    HBlank,
    VCounter,
}

/// Events the PPU can notify DMAs about.
pub enum PpuDmaReason {
    VBlank,
    HBlank,
}
