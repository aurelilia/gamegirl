use crate::{
    gga::{graphics::Ppu, GameGirlAdv},
    scheduler::Kind,
};
use serde::{Deserialize, Serialize};
use AdvEvent::*;

/// All scheduler events on the GGA.
#[derive(Copy, Clone, Deserialize, Serialize)]
#[repr(u16)]
pub enum AdvEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    PauseEmulation,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
}

impl AdvEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&self, gg: &mut GameGirlAdv, late_by: u32) {
        match self {
            PauseEmulation => gg.unpaused = false,
            PpuEvent(evt) => Ppu::handle_event(gg, *evt, late_by),
        }
    }
}

// Not implementing this breaks Scheduler::default for SOME reason
impl Default for AdvEvent {
    fn default() -> Self {
        PauseEmulation
    }
}

impl Kind for AdvEvent {}

/// Events the PPU generates.
#[derive(Copy, Clone, Deserialize, Serialize)]
#[repr(u16)]
pub enum PpuEvent {
    /// Start of HBlank.
    HblankStart,
    /// End of HBlank, which is the start of the next scanline.
    HblankEnd,
}
