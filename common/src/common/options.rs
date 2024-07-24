// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::vec;

use super::audio::AudioSampler;

/// Options that are used by the GUI and shared between all systems.
/// These can be changed at runtime.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EmulateOptions {
    /// If the audio samples produced by [produce_samples] should be in reversed
    /// order. `true` while rewinding.
    pub invert_audio_samples: bool,
    /// Speed multiplier the system should run at.
    /// ex. 1x is regular speed, 2x is double speed.
    /// Affects [advance_delta] and sound sample output.
    pub speed_multiplier: usize,
}

impl Default for EmulateOptions {
    fn default() -> Self {
        Self {
            invert_audio_samples: false,
            speed_multiplier: 1,
        }
    }
}

/// Configuration used when initializing the system.
/// These options don't change at runtime.
#[derive(Clone)]
#[cfg_attr(feature = "serde_config", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde_config", serde(default))]
pub struct SystemConfig {
    /// How to handle CGB mode.
    pub mode: CgbMode,
    /// If save states should be compressed.
    pub compress_savestates: bool,
    /// If CGB colours should be corrected.
    pub cgb_colour_correction: bool,
    /// If the 'bootrom' or BIOS should be skipped, where applicable.
    pub skip_bootrom: bool,
    /// If the system should start running immediately when loading a ROM.
    pub run_on_open: bool,
    /// Audio volume multiplier
    pub volume: f32,
    /// Audio volume multiplier while fast forwarding
    pub volume_ff: f32,
    /// Audio output sample rate
    pub sample_rate: usize,
    /// Audio resampler
    pub resampler: AudioSampler,
    /// If the interpreter should cache
    pub cached_interpreter: bool,
    /// If the PPU should run on a sepearate thread.
    pub threaded_ppu: bool,
    /// BIOSes to use / load.
    pub bioses: Vec<ConsoleBios>,
}

impl SystemConfig {
    /// Get the BIOS for a given console ID.
    pub fn get_bios(&self, console_id: &str) -> Option<&[u8]> {
        self.bioses
            .iter()
            .find(|bios| bios.console_id == console_id)
            .and_then(|bios| bios.bios.as_deref())
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            mode: CgbMode::Prefer,
            compress_savestates: false,
            cgb_colour_correction: false,
            skip_bootrom: false,
            run_on_open: true,
            volume: 0.5,
            volume_ff: 0.25,
            sample_rate: 48000,
            resampler: AudioSampler::Cubic,
            cached_interpreter: true,
            // WASM doesn't do threads
            threaded_ppu: !cfg!(target_arch = "wasm32"),
            bioses: vec![
                ConsoleBios {
                    console_id: "dmg".into(),
                    console_name: "Game Boy".into(),
                    bios: None,
                },
                ConsoleBios {
                    console_id: "cgb".into(),
                    console_name: "Game Boy Color".into(),
                    bios: None,
                },
                ConsoleBios {
                    console_id: "agb".into(),
                    console_name: "Game Boy Advance".into(),
                    bios: None,
                },
                ConsoleBios {
                    console_id: "nds7".into(),
                    console_name: "DS (ARM7)".into(),
                    bios: None,
                },
                ConsoleBios {
                    console_id: "nds9".into(),
                    console_name: "DS (ARM9)".into(),
                    bios: None,
                },
                ConsoleBios {
                    console_id: "ndsfw".into(),
                    console_name: "DS (Firmware)".into(),
                    bios: None,
                },
                ConsoleBios {
                    console_id: "psx".into(),
                    console_name: "PlayStation".into(),
                    bios: None,
                },
            ],
        }
    }
}

/// How to handle CGB mode depending on cart compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde_config", derive(serde::Deserialize, serde::Serialize))]
pub enum CgbMode {
    /// Always run in CGB mode, even when the cart does not support it.
    /// If it does not, it is run in DMG compatibility mode, just like on a
    /// real CGB.
    Always,
    /// If the cart has CGB support, run it as CGB; if not, don't.
    Prefer,
    /// Never run the cart in CGB mode unless it requires it.
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde_config", derive(serde::Deserialize, serde::Serialize))]
pub struct ConsoleBios {
    pub console_id: String,
    pub console_name: String,
    pub bios: Option<Vec<u8>>,
}
