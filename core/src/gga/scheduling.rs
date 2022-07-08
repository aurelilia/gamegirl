use serde::{Deserialize, Serialize};
use AdvEvent::*;

use crate::{
    gga::{audio::Apu, graphics::Ppu, timer::Timers, GameGirlAdv},
    ggc::io::apu::GenApuEvent,
    scheduler::Kind,
};

/// All scheduler events on the GGA.
#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[repr(u16)]
pub enum AdvEvent {
    /// Pause the emulation. Used by `advance_delta` to advance by a certain
    /// amount.
    PauseEmulation,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
    /// An event handled by the APU.
    ApuEvent(ApuEvent),
    /// A timer overflow.
    TimerOverflow(u8),
}

impl AdvEvent {
    /// Handle the event by delegating to the appropriate handler.
    pub fn dispatch(&self, gg: &mut GameGirlAdv, late_by: u32) {
        match self {
            PauseEmulation => gg.ticking = false,
            PpuEvent(evt) => Ppu::handle_event(gg, *evt, late_by),
            ApuEvent(evt) => {
                let time = Apu::handle_event(gg, *evt, late_by);
                gg.scheduler.schedule(*self, time);
            }
            TimerOverflow(idx) => Timers::handle_overflow_event(gg, *idx, late_by),
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
#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[repr(u16)]
pub enum PpuEvent {
    /// Start of HBlank.
    HblankStart,
    /// Set HBlank flag in DISPSTAT (this is delayed by 46 cycles)
    SetHblank,
    /// End of HBlank, which is the start of the next scanline.
    HblankEnd,
}

/// Events the APU generates.
#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[repr(u16)]
pub enum ApuEvent {
    /// Event from the generic CGB APU.
    Gen(GenApuEvent),
    /// Tick the CGB sequencer.
    Sequencer,
    /// Push a sample to the output.
    PushSample,
}
