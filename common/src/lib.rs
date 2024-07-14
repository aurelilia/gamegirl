// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![feature(btree_cursors)]

use std::{any::Any, cmp::Ordering, mem};

use common::debugger::Width;
pub use common::Common;
pub use components::scheduler::{Time, TimeS};
use components::storage::GameSave;

pub mod common;
pub mod components;
#[macro_use]
pub mod macros;
pub mod numutil;
#[cfg(feature = "serde")]
pub mod serialize;

/// Maximum pointer size used by any system. This is used in some places, like
/// the debugger, to avoid needing to use generic parameters.
pub type Pointer = u32;

/// Colour type used by the system's PPUs for image data.
/// This type is analogus to egui's `Color32`, which allows the GUI to
/// simply `mem::transmute` it without having to perform any explicit
/// conversion. Additionally, due to this approach the core crate does not need
/// to depend on the rather heavy egui.
pub type Colour = [u8; 4];

pub trait Core: Send + Sync {
    /// Advance by one step, where step is system-defined.
    fn advance(&mut self);
    /// Advance the system clock by _at least_ the given delta in seconds.
    /// Might advance more.
    fn advance_delta(&mut self, delta: f32);
    /// Reset the console, while keeping the current cartridge inserted.
    fn reset(&mut self);
    /// Skip BIOS, bootroms, or similar; immediately boot inserted game.
    fn skip_bootrom(&mut self);

    /// Create a save state that can be loaded with [load_state].
    fn save_state(&mut self) -> Vec<u8>;
    /// Load a state produced by [save_state].
    /// Will restore the current cartridge and debugger.
    fn load_state(&mut self, state: &[u8]);

    /// Get the current system time.
    fn get_time(&self) -> Time;
    /// Returns the screen size for the current system.
    fn screen_size(&self) -> [usize; 2];
    /// Returns the output audio sample rate for the current system.
    fn wanted_sample_rate(&self) -> u32;
    /// Make a save for the game to be put to disk.
    fn make_save(&self) -> Option<GameSave>;

    /// Get the value at the given memory address.
    /// The width parameter specifies the size of the value to read.
    /// Remaining bits are zero.
    fn get_memory(&self, _addr: u32, _width: Width) -> u32 {
        unimplemented!("Not implemented for this core")
    }
    /// Search for the given value in memory.
    /// The width parameter specifies the size of the value to search for.
    /// Returns a list of matching addresses.
    /// Note that unaligned values will not be checked; the exact meaning
    /// of unaligned is platform-specific.
    fn search_memory(&self, _value: u32, _width: Width, _kind: Ordering) -> Vec<u32> {
        vec![]
    }
    /// Get the value of all registers. Exact meaning is platform-specific.
    fn get_registers(&self) -> Vec<usize> {
        unimplemented!("Not implemented for this core")
    }
    /// Get the ROM currently loaded.
    fn get_rom(&self) -> Vec<u8>;
    /// Set the value at the given memory address.
    /// The width parameter specifies the size of the value to write.
    /// Remaining bits are ignored.
    fn set_memory(&mut self, _addr: u32, _value: u32, _width: Width) {
        unimplemented!("Not implemented for this core")
    }

    fn c(&self) -> &Common;
    fn c_mut(&mut self) -> &mut Common;
    fn as_any(&mut self) -> &mut dyn Any;

    fn produce_frame(&mut self) -> Option<Vec<Colour>> {
        while self.c().debugger.running && self.c_mut().video_buffer.pop().is_none() {
            self.advance();
        }

        // Do it twice: Color buffer will be empty after a save state load,
        // we need to render one frame in full
        while self.c().debugger.running && !self.c().video_buffer.has_frame() {
            self.advance();
        }
        self.c_mut().video_buffer.pop()
    }

    fn produce_samples(&mut self, samples: &mut [f32]) {
        if !self.c().debugger.running {
            samples.fill(0.0);
            return;
        }

        let target = samples.len() * self.c().options.speed_multiplier;
        while self.c().audio_buffer.len() < target {
            if !self.c().debugger.running {
                samples.fill(0.0);
                return;
            }
            self.advance();
        }

        let mut buffer = mem::take(&mut self.c_mut().audio_buffer);
        if self.c().options.invert_audio_samples {
            // If rewinding, truncate and get rid of any excess samples to prevent
            // audio samples getting backed up
            for (src, dst) in buffer.into_iter().zip(samples.iter_mut().rev()) {
                *dst = src * self.c().config.volume_ff;
            }
        } else {
            // Otherwise, store any excess samples back in the buffer for next time
            // while again not storing too many to avoid backing up.
            // This way can cause clipping if the console produces audio too fast,
            // however this is preferred to audio falling behind and eating
            // a lot of memory.
            for sample in buffer.drain(target..) {
                self.c_mut().audio_buffer.push(sample);
            }
            if self.c().audio_buffer.len() > self.wanted_sample_rate() as usize / 2 {
                log::warn!("Audio samples are backing up! Truncating");
                self.c_mut().audio_buffer.truncate(100);
            }

            let volume = if self.c().options.speed_multiplier == 1 {
                self.c().config.volume
            } else {
                self.c().config.volume_ff
            };
            for (src, dst) in buffer
                .into_iter()
                .step_by(self.c().options.speed_multiplier)
                .zip(samples.iter_mut())
            {
                *dst = src * volume;
            }
        }
    }
}
