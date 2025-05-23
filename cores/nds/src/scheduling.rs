// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::{components::scheduler::Kind, TimeS};
use NdsEvent::*;

use crate::{
    cpu::NDS9_CLOCK,
    graphics::Gpu,
    hw::{
        audio::Apu,
        dma::{Dma, Dmas},
        timer::Timers,
    },
    Nds,
};

#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum NdsEvent {
    PauseEmulation,
    /// An event handled by the PPU.
    PpuEvent(PpuEvent),
    /// An event handled by the APU.
    ApuEvent(ApuEvent),
    /// A timer overflow.
    TimerOverflow {
        timer: u8,
        is_arm9: bool,
    },
    /// Update the keypad.
    UpdateKeypad,
    /// Event handled by the cart.
    CartEvent(CartEvent),
}

impl NdsEvent {
    pub fn dispatch(self, ds: &mut Nds, late_by: TimeS) {
        match self {
            PauseEmulation => ds.c.in_tick = false,
            PpuEvent(evt) => {
                Gpu::handle_event(&mut ds.nds9(), evt, late_by);
            }
            ApuEvent(evt) => {
                let time = Apu::handle_event(ds, evt, late_by);
                ds.scheduler.schedule(self, time);
            }
            TimerOverflow { timer, is_arm9 } if is_arm9 => {
                Timers::handle_overflow_event(ds.nds9(), timer, late_by)
            }
            TimerOverflow { timer, .. } => Timers::handle_overflow_event(ds.nds7(), timer, late_by),
            UpdateKeypad => {
                Nds::check_keycnt(ds.nds7());
                Nds::check_keycnt(ds.nds9());
                ds.scheduler
                    .schedule(NdsEvent::UpdateKeypad, (NDS9_CLOCK as f64 / 120.0) as TimeS);
            }
            CartEvent(evt) => {
                if ds.cart.handle_evt(evt) {
                    Dmas::update_all(ds.nds7(), crate::hw::dma::Reason::CartridgeReady);
                    Dmas::update_all(ds.nds9(), crate::hw::dma::Reason::CartridgeReady);
                }
            }
        }
    }
}

impl Kind for NdsEvent {}

impl Default for NdsEvent {
    fn default() -> Self {
        Self::PauseEmulation
    }
}

/// Events the APU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum ApuEvent {
    /// Push a sample to the output.
    PushSample,
}

/// Events the PPU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum PpuEvent {
    /// Start of HBlank.
    HblankStart,
    /// Set HBlank flag in DISPSTAT (this is delayed by 46 cycles)
    SetHblank,
    /// End of HBlank, which is the start of the next scanline.
    HblankEnd,
}

/// Events the cart generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum CartEvent {
    /// AUXSPIDATA transfer completed
    SpiDataComplete,
    /// ROM is ready
    RomTransferReady,
}
