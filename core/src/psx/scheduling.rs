use serde::{Deserialize, Serialize};

use crate::{psx::PlayStation, scheduler::Kind};

#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum PsxEvent {
    PauseEmulation,
}

impl PsxEvent {
    pub fn dispatch(&self, _ps: &mut PlayStation, _late_by: i32) {}
}

impl Kind for PsxEvent {}

impl Default for PsxEvent {
    fn default() -> Self {
        Self::PauseEmulation
    }
}
