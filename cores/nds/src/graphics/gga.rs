// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use arm_cpu::Cpu;
use common::misc::EmulateOptions;
use gga_ppu::{
    interface::{PpuDmaReason, PpuInterrupt, PpuSystem},
    scheduling::PpuEvent,
    threading::{GgaPpu, PpuMmio},
};

use crate::{dma::Reason, Dmas, Nds7, Nds9, NdsEvent};

impl PpuSystem for Nds7 {
    const W: usize = 256;
    const H: usize = 192;
    const VBLANK_END: usize = Self::H + 71;

    fn ppu(&mut self) -> &mut GgaPpu<Self>
    where
        [(); Self::W * Self::H]:,
    {
        &mut self.ppu.ppu_a
    }

    fn ppur(&self) -> &GgaPpu<Self>
    where
        [(); Self::W * Self::H]:,
    {
        &self.ppu.ppu_a
    }

    fn options(&mut self) -> &mut EmulateOptions {
        &mut self.options
    }

    fn ppu_mmio(&mut self) -> PpuMmio {
        let mut mmio = [0; 0x56 / 2];
        mmio.copy_from_slice(&self.memory.mmio9[..0x56 / 2]);
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
        self.scheduler.schedule(NdsEvent::PpuEvent(evt), at);
    }

    fn frame_finished(&mut self) {
        #[cfg(feature = "serde")]
        {
            let state = self.save_state();
            (self.options.frame_finished)(state);
        }
    }
}

impl PpuSystem for Nds9 {
    const W: usize = 256;
    const H: usize = 192;
    const VBLANK_END: usize = Self::H + 71;

    fn ppu(&mut self) -> &mut GgaPpu<Self>
    where
        [(); Self::W * Self::H]:,
    {
        &mut self.ppu.ppu_b
    }

    fn ppur(&self) -> &GgaPpu<Self>
    where
        [(); Self::W * Self::H]:,
    {
        &self.ppu.ppu_b
    }

    fn options(&mut self) -> &mut EmulateOptions {
        &mut self.options
    }

    fn ppu_mmio(&mut self) -> PpuMmio {
        let mut mmio = [0; 0x56 / 2];
        mmio.copy_from_slice(&self.memory.mmio9[0x500..0x528]);
        // Account for unmirrored DISPSTAT and VCOUNT
        mmio[0x2] = self.memory.mmio9[0x2];
        mmio[0x3] = self.memory.mmio9[0x3];
        mmio
    }

    fn request_interrupt(&mut self, _int: PpuInterrupt) {
        // Do nothing, let PPU A do the interrupt
    }

    fn notify_dma(&mut self, _reason: PpuDmaReason) {
        // Do nothing, let PPU A notify the DMAs
    }

    fn schedule(&mut self, _evt: PpuEvent, _at: i32) {
        // Do nothing, let PPU A schedule
    }

    fn frame_finished(&mut self) {
        // Do nothing, let PPU A handle it
    }
}

#[cfg(feature = "threaded-ppu")]
mod thread {
    use std::sync::MutexGuard;

    use gga_ppu::Ppu;

    use crate::{Nds, Nds7, Nds9};

    impl Nds {
        #[inline]
        pub(crate) fn ppu_a(&mut self) -> MutexGuard<Ppu<Nds7>> {
            self.ppu.ppu_a.ppu.lock().unwrap()
        }

        #[inline]
        pub(crate) fn ppu_a_nomut(&self) -> MutexGuard<Ppu<Nds7>> {
            self.ppu.ppu_a.ppu.lock().unwrap()
        }

        #[inline]
        pub(crate) fn ppu_b(&mut self) -> MutexGuard<Ppu<Nds9>> {
            self.ppu.ppu_b.ppu.lock().unwrap()
        }

        #[inline]
        pub(crate) fn ppu_b_nomut(&self) -> MutexGuard<Ppu<Nds9>> {
            self.ppu.ppu_b.ppu.lock().unwrap()
        }
    }
}

#[cfg(not(feature = "threaded-ppu"))]
mod thread {
    use gga_ppu::{threading::GgaPpu, Ppu};

    use crate::{Nds, Nds7, Nds9};

    impl Nds {
        #[inline]
        pub(crate) fn ppu_a(&mut self) -> &mut GgaPpu<Nds7> {
            &mut self.ppu.ppu_a
        }

        #[inline]
        pub(crate) fn ppu_a_nomut(&self) -> &GgaPpu<Nds7> {
            &self.ppu.ppu_a
        }

        #[inline]
        pub(crate) fn ppu_b(&mut self) -> &mut GgaPpu<Nds9> {
            &mut self.ppu.ppu_b
        }

        #[inline]
        pub(crate) fn ppu_b_nomut(&self) -> &GgaPpu<Nds9> {
            &self.ppu.ppu_b
        }
    }
}
