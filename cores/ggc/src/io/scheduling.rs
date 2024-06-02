// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{components::scheduler::Kind, TimeS};
use GGEvent::*;

use super::joypad::Joypad;
use crate::{
    io::{dma, dma::Hdma, ppu::Ppu},
    GameGirl,
};

/// All scheduler events on the GG.
#[derive(Copy, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum GGEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    #[default]
    PauseEmulation,
    /// Update button inputs.
    UpdateKeypad,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
    /// A DMA transfer completion.
    DMAFinish,
    /// Advance HDMA transfer.
    HdmaTransferStep,
    /// A GDMA transfer.
    GdmaTransfer,
}

impl GGEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&self, gg: &mut GameGirl, late_by: TimeS) {
        match self {
            PauseEmulation => gg.ticking = false,
            UpdateKeypad => Joypad::update(gg),
            PpuEvent(evt) => Ppu::handle_event(gg, *evt, late_by),
            DMAFinish => dma::do_oam_dma(gg),
            HdmaTransferStep => Hdma::handle_hdma(gg),
            GdmaTransfer => Hdma::handle_gdma(gg),
        }
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
