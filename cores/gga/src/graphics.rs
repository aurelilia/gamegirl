// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use arm_cpu::Cpu;
use gga_ppu::{
    interface::{PpuDmaReason, PpuInterrupt, PpuSystem},
    scheduling::PpuEvent,
    threading::{GgaPpu, PpuMmio},
    Ppu,
};

use crate::{dma::Reason, AdvEvent, Dmas, GameGirlAdv};

impl PpuSystem for GameGirlAdv {
    const W: usize = 240;
    const H: usize = 160;
    const VBLANK_END: usize = 228;

    fn ppu(&mut self) -> &mut GgaPpu<Self>
    where
        [(); Self::W * Self::H]:,
    {
        &mut self.ppu
    }

    fn ppur(&self) -> &GgaPpu<Self>
    where
        [(); Self::W * Self::H]:,
    {
        &self.ppu
    }

    fn ppu_mmio(&mut self) -> PpuMmio {
        let mut mmio = [0; 0x56 / 2];
        mmio.copy_from_slice(&self.memory.mmio[..0x56 / 2]);
        mmio
    }

    fn request_interrupt(&mut self, int: PpuInterrupt) {
        Cpu::request_interrupt_idx(self, int as u16)
    }

    fn notify_dma(&mut self, reason: PpuDmaReason) {
        let reason = match reason {
            PpuDmaReason::VBlank => Reason::VBlank,
            PpuDmaReason::HBlank => Reason::HBlank,
        };
        Dmas::update_all(self, reason);
    }

    fn schedule(&mut self, evt: PpuEvent, at: i32) {
        self.scheduler.schedule(AdvEvent::PpuEvent(evt), at);
    }
}

#[cfg(not(feature = "threaded-ppu"))]
impl GameGirlAdv {
    #[inline]
    pub fn ppu(&mut self) -> &mut GgaPpu<GameGirlAdv> {
        &mut self.ppu
    }

    #[inline]
    pub fn ppu_nomut(&self) -> &GgaPpu<GameGirlAdv> {
        &self.ppu
    }
}

#[cfg(feature = "threaded-ppu")]
impl GameGirlAdv {
    #[inline]
    pub fn ppu(&mut self) -> std::sync::MutexGuard<Ppu<Self>> {
        self.ppu.ppu.lock().unwrap()
    }

    #[inline]
    pub fn ppu_nomut(&self) -> std::sync::MutexGuard<Ppu<Self>> {
        self.ppu.ppu.lock().unwrap()
    }
}
