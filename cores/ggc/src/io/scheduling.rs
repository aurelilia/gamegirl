// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::components::{apu_psg::GenApuEvent, scheduler::Kind};
use GGEvent::*;

use crate::{
    cpu::Interrupt,
    io::{
        addr::{TIMA, TMA},
        apu::Apu,
        dma,
        dma::Hdma,
        ppu::Ppu,
        timer::Timer,
    },
    GameGirl,
};

/// All scheduler events on the GG.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum GGEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    PauseEmulation,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
    /// An event handled by the APU.
    ApuEvent(ApuEvent),
    /// A DMA transfer completion.
    DMAFinish,
    /// Advance HDMA transfer.
    HdmaTransferStep,
    /// A GDMA transfer.
    GdmaTransfer,
    /// A timer overflow.
    TimerOverflow,
    /// A TMA reload. This only happens 4 t-cycles after timer overflow, hence
    /// the separate event.
    TmaReload,
}

impl GGEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&self, gg: &mut GameGirl, late_by: i32) {
        match self {
            PauseEmulation => gg.ticking = false,
            PpuEvent(evt) => Ppu::handle_event(gg, *evt, late_by),
            ApuEvent(evt) => Apu::handle_event(gg, *evt, late_by),
            DMAFinish => dma::do_oam_dma(gg),
            HdmaTransferStep => Hdma::handle_hdma(gg),
            GdmaTransfer => Hdma::handle_gdma(gg),
            TimerOverflow => Timer::on_overflow(gg, late_by),
            TmaReload => {
                gg[TIMA] = gg[TMA];
                gg.request_interrupt(Interrupt::Timer);
            }
        }
    }
}

// Not implementing this breaks Scheduler::default for SOME reason
impl Default for GGEvent {
    fn default() -> Self {
        PauseEmulation
    }
}

impl Kind for GGEvent {}

/// Events the PPU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum PpuEvent {
    OamScanEnd,
    UploadEnd,
    HblankEnd,
    VblankEnd,
    // This happens a little after HBlank end
    LYIncrement,
}

impl PpuEvent {
    pub(crate) fn ordinal(self) -> u8 {
        // ehhh
        match self {
            PpuEvent::HblankEnd => 0,
            PpuEvent::VblankEnd => 1,
            PpuEvent::OamScanEnd => 2,
            PpuEvent::UploadEnd => 3,
            PpuEvent::LYIncrement => panic!("Not applicable!"),
        }
    }
}

/// Events the APU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum ApuEvent {
    /// Push a sample to the output.
    PushSample,
    /// Tick the sequencer.
    TickSequencer,
    /// Event from the inner generic APU.
    Gen(GenApuEvent),
}
