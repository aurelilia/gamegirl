// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::IndexMut;

use common::misc::EmulateOptions;

use crate::{
    threading::{GgaPpu, PpuMmio},
    PpuEvent,
};

pub trait PpuSystem: IndexMut<u32, Output = u16> + Sized + 'static {
    const W: usize;
    const H: usize;
    const VBLANK_END: usize;

    fn ppu(&mut self) -> &mut GgaPpu<Self>
    where
        [(); Self::W * Self::H]:;
    fn ppur(&self) -> &GgaPpu<Self>
    where
        [(); Self::W * Self::H]:;
    fn options(&mut self) -> &mut EmulateOptions;
    fn ppu_mmio(&mut self) -> PpuMmio;

    fn request_interrupt(&mut self, int: PpuInterrupt);
    fn notify_dma(&mut self, reason: PpuDmaReason);
    fn schedule(&mut self, evt: PpuEvent, at: i32);
    fn frame_finished(&mut self);
}

#[repr(C)]
pub enum PpuInterrupt {
    VBlank,
    HBlank,
    VCounter,
}

pub enum PpuDmaReason {
    VBlank,
    HBlank,
}
