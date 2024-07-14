// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use debugger::Debugger;
use input::Input;
use options::{EmulateOptions, SystemConfig};
use video::FrameBuffer;

pub mod debugger;
pub mod input;
pub mod options;
pub mod video;

/// Common fields shared by all systems.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Common {
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub debugger: Debugger,
    pub options: EmulateOptions,
    pub config: SystemConfig,
    pub in_tick: bool,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub video_buffer: FrameBuffer,
    pub audio_buffer: Vec<f32>,
    pub input: Input,
}

impl Common {
    pub fn with_config(config: SystemConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn restore_from(&mut self, old: Self) {
        self.debugger = old.debugger;
        self.options = old.options;
        self.config = old.config;
    }
}
