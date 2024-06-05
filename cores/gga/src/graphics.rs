// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use arm_cpu::Cpu;
use common::TimeS;
use gga_ppu::{
    interface::{PpuDmaReason, PpuInterrupt, PpuSystem},
    scheduling::PpuEvent,
    threading::{GgaPpu, PpuMmio},
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

    fn schedule(&mut self, evt: PpuEvent, at: TimeS) {
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
    pub fn ppu(&mut self) -> std::sync::MutexGuard<gga_ppu::Ppu<Self>> {
        self.ppu.ppu.lock().unwrap()
    }

    #[inline]
    pub fn ppu_nomut(&self) -> std::sync::MutexGuard<gga_ppu::Ppu<Self>> {
        self.ppu.ppu.lock().unwrap()
    }
}
