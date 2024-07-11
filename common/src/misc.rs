// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use crate::components::input::Input;

/// Options that are used by the GUI and shared between all systems.
/// These can be changed at runtime.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EmulateOptions {
    /// If there is a ROM loaded / cartridge inserted.
    pub rom_loaded: bool,
    /// If the audio samples produced by [produce_samples] should be in reversed
    /// order. `true` while rewinding.
    pub invert_audio_samples: bool,
    /// Speed multiplier the system should run at.
    /// ex. 1x is regular speed, 2x is double speed.
    /// Affects [advance_delta] and sound sample output.
    pub speed_multiplier: usize,
    /// Input subsystem to use.
    pub input: Input,
}

impl Default for EmulateOptions {
    fn default() -> Self {
        Self {
            rom_loaded: false,
            invert_audio_samples: false,
            speed_multiplier: 1,
            input: Input::default(),
        }
    }
}

/// Configuration used when initializing the system.
/// These options don't change at runtime.
#[derive(Clone)]
#[cfg_attr(feature = "serde_config", derive(serde::Deserialize, serde::Serialize))]
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
    /// If the interpreter should cache
    pub cached_interpreter: bool,
    /// If the PPU should run on a sepearate thread.
    pub threaded_ppu: bool,
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
            cached_interpreter: true,
            // WASM doesn't do threads
            threaded_ppu: !cfg!(target_arch = "wasm32"),
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

/// Serialize an object that can be loaded with [deserialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(all(feature = "serde", feature = "zstd"))]
pub fn serialize<T: serde::Serialize>(thing: &T, with_zstd: bool) -> Vec<u8> {
    if with_zstd {
        let mut dest = vec![];
        let mut writer = zstd::stream::Encoder::new(&mut dest, 3).unwrap();
        bincode::serialize_into(&mut writer, thing).unwrap();
        writer.finish().unwrap();
        dest
    } else {
        bincode::serialize(thing).unwrap()
    }
}

/// Deserialize an object that was made with [serialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(all(feature = "serde", feature = "zstd"))]
pub fn deserialize<T: serde::de::DeserializeOwned>(state: &[u8], with_zstd: bool) -> T {
    if with_zstd {
        let decoder = zstd::stream::Decoder::new(state).unwrap();
        bincode::deserialize_from(decoder).unwrap()
    } else {
        bincode::deserialize(state).unwrap()
    }
}
/// Serialize an object that can be loaded with [deserialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(all(feature = "serde", not(feature = "zstd")))]
pub fn serialize<T: serde::Serialize>(thing: &T, _with_zstd: bool) -> Vec<u8> {
    bincode::serialize(thing).unwrap()
}

/// Deserialize an object that was made with [serialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(all(feature = "serde", not(feature = "zstd")))]
pub fn deserialize<T: serde::de::DeserializeOwned>(state: &[u8], with_zstd: bool) -> T {
    bincode::deserialize(state).unwrap()
}
