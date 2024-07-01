// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![feature(btree_cursors)]

use std::any::Any;

pub use components::scheduler::{Time, TimeS};
use components::storage::GameSave;
use misc::{EmulateOptions, SystemConfig};

pub mod components;
pub mod misc;
#[macro_use]
pub mod macros;
pub mod numutil;

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
    /// Get the running status of the console, and allow modifying it.
    fn is_running(&mut self) -> &mut bool;
    /// Skip BIOS, bootroms, or similar; immediately boot inserted game.
    fn skip_bootrom(&mut self);

    // Take the last frame output by the graphics hardware.
    fn last_frame(&mut self) -> Option<Vec<Colour>>;
    fn options(&mut self) -> &mut EmulateOptions;
    fn config(&self) -> &SystemConfig;
    fn config_mut(&mut self) -> &mut SystemConfig;

    /// Get the current system time.
    fn get_time(&self) -> Time;
    /// Returns the screen size for the current system.
    fn screen_size(&self) -> [usize; 2];
    /// Returns the output audio sample rate for the current system.
    fn wanted_sample_rate(&self) -> u32;
    /// Make a save for the game to be put to disk.
    fn make_save(&self) -> Option<GameSave>;

    /// Get the value at the given memory address.
    fn get_memory(&self, _addr: usize) -> u8 {
        unimplemented!("Not implemented for this core")
    }
    /// Get the value of all registers. Exact meaning is platform-specific.
    fn get_registers(&self) -> Vec<usize> {
        unimplemented!("Not implemented for this core")
    }
    /// Get values written to serial. Exact meaning is platform-specific.
    fn get_serial(&self) -> &[u8] {
        unimplemented!("Not implemented for this core")
    }

    fn as_any(&mut self) -> &mut dyn Any;

    fn get_rom(&self) -> Vec<u8>;
}
