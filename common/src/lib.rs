// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![feature(btree_cursors)]

use std::{any::Any, cmp::Ordering};

pub use components::scheduler::{Time, TimeS};
use components::storage::GameSave;
use misc::{EmulateOptions, SystemConfig};
use numutil::NumExt;

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
    fn save_state(&mut self) -> Vec<u8>;
    /// Load a state produced by [save_state].
    /// Will restore the current cartridge and debugger.
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
    /// Get values written to serial. Exact meaning is platform-specific.
    fn get_serial(&self) -> &[u8] {
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

    fn as_any(&mut self) -> &mut dyn Any;
}

/// Width of a value to be read/written from memory.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    #[default]
    Byte,
    Halfword,
    Word,
}

impl Width {
    pub fn size(&self) -> usize {
        match self {
            Width::Byte => 1,
            Width::Halfword => 2,
            Width::Word => 4,
        }
    }

    pub fn mask(&self) -> u32 {
        match self {
            Width::Byte => 0xFF,
            Width::Halfword => 0xFFFF,
            Width::Word => 0xFFFFFFFF,
        }
    }
}

pub fn search_array(
    matches: &mut Vec<u32>,
    arr: &[u8],
    offset: u32,
    value: u32,
    width: Width,
    kind: Ordering,
) {
    for (i, chunk) in arr.chunks_exact(width.size()).enumerate() {
        let chunk = match width {
            Width::Byte => chunk[0].u32(),
            Width::Halfword => u16::from_le_bytes([chunk[0], chunk[1]]).u32(),
            Width::Word => u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        };
        if chunk.cmp(&value) == kind {
            matches.push(offset + i.u32() * width.size().u32());
        }
    }
}
