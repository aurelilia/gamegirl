use crate::{gga::GameGirlAdv, scheduler::Kind};
use serde::{Deserialize, Serialize};
use AdvEvent::*;

#[derive(Copy, Clone, Deserialize, Serialize)]
#[repr(u16)]
pub enum AdvEvent {
    PauseEmulation,
}

impl AdvEvent {
    pub fn dispatch(&self, gg: &mut GameGirlAdv, _late_by: u32) {
        match self {
            PauseEmulation => gg.unpaused = false,
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
