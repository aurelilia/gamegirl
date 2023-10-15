// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::any::Any;

use components::storage::GameSave;
use misc::{Button, EmulateOptions, SystemConfig};

pub mod components;
pub mod misc;
#[macro_use]
pub mod macros;
pub mod numutil;

/// Audio sample rate of all emulated systems.
pub const SAMPLE_RATE: u32 = 48000;

/// For debugging: If instruction-level tracing output should be printed.
pub const TRACING: bool = false;

/// Colour type used by the system's PPUs for image data.
/// This type is analogus to egui's `Color32`, which allows the GUI to
/// simply `mem::transmute` it without having to perform any explicit
/// conversion. Additionally, due to this approach the core crate does not need
/// to depend on the rather heavy egui.
pub type Colour = [u8; 4];

pub trait Core: Send + Sync {
    /// Advance the system clock by the given delta in seconds.
    /// Might advance a few clocks more.
    fn advance_delta(&mut self, delta: f32);
    /// Step until the PPU has finished producing the current frame.
    /// Only used for rewinding since it causes audio desync very easily.
    fn produce_frame(&mut self) -> Option<Vec<Colour>>;
    /// Produce the next audio samples and write them to the given buffer.
    /// Writes zeroes if the system is not currently running
    /// and no audio should be played.
    fn produce_samples(&mut self, buffer: &mut [f32]);

    /// Create a save state that can be loaded with [load_state].
    #[cfg(feature = "serde")]
    fn save_state(&mut self) -> Vec<u8>;
    /// Load a state produced by [save_state].
    /// Will restore the current cartridge and debugger.
    #[cfg(feature = "serde")]
    fn load_state(&mut self, state: &[u8]);

    /// Advance by one step, where step is system-defined.
    fn advance(&mut self);
    /// Reset the console, while keeping the current cartridge inserted.
    fn reset(&mut self);
    /// Skip BIOS, bootroms, or similar; immediately boot inserted game.
    fn skip_bootrom(&mut self);

    fn last_frame(&mut self) -> Option<Vec<Colour>>;
    fn options(&mut self) -> &mut EmulateOptions;
    fn config(&self) -> &SystemConfig;
    fn config_mut(&mut self) -> &mut SystemConfig;

    /// Set a button on the joypad.
    fn set_button(&mut self, btn: Button, pressed: bool);
    /// Returns the screen size for the current system.
    fn screen_size(&self) -> [usize; 2];
    /// Make a save for the game to be put to disk.
    fn make_save(&self) -> Option<GameSave>;

    fn as_any(&mut self) -> &mut dyn Any;
}

impl dyn Core {}
