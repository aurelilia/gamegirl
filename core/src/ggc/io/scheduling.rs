use crate::{
    ggc::{
        io::{
            apu::{GGApu, GenApuEvent},
            dma,
            dma::Hdma,
            ppu::Ppu,
        },
        GameGirl,
    },
    scheduler::Kind,
};
use serde::{Deserialize, Serialize};
use GGEvent::*;

/// All scheduler events on the GG.
#[derive(Copy, Clone, PartialEq, Deserialize, Serialize)]
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
}

impl GGEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&self, gg: &mut GameGirl, late_by: u32) {
        match self {
            PauseEmulation => gg.unpaused = false,
            PpuEvent(evt) => Ppu::handle_event(gg, *evt, late_by),
            ApuEvent(evt) => GGApu::handle_event(gg, *evt, late_by),
            DMAFinish => dma::do_oam_dma(gg),
            HdmaTransferStep => Hdma::handle_hdma(gg),
            GdmaTransfer => Hdma::handle_gdma(gg),
            _ => panic!("aaaaaa"),
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
#[derive(Copy, Clone, PartialEq, Deserialize, Serialize)]
#[repr(u16)]
pub enum PpuEvent {
    OamScanEnd,
    UploadEnd,
    HblankEnd,
    VblankEnd,
}

impl PpuEvent {
    pub(crate) fn ordinal(self) -> u8 {
        // ehhh
        match self {
            PpuEvent::HblankEnd => 0,
            PpuEvent::VblankEnd => 1,
            PpuEvent::OamScanEnd => 2,
            PpuEvent::UploadEnd => 3,
        }
    }
}

/// Events the APU generates.
#[derive(Copy, Clone, PartialEq, Deserialize, Serialize)]
#[repr(u16)]
pub enum ApuEvent {
    /// Push a sample to the output.
    PushSample,
    /// Event from the inner generic APU.
    Gen(GenApuEvent),
}
