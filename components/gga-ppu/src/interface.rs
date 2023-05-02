// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::IndexMut;

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
    fn schedule(&mut self, evt: PpuEvent, at: i32);
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
