// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{any::Any, cell::UnsafeCell, cmp::Ordering};

pub use common::Common;
use common::{debugger::Width, options::SystemConfig};
pub use components::scheduler::{Time, TimeS};
use components::storage::{GameCart, GameSave};

pub mod common;
pub mod components;
#[macro_use]
pub mod macros;
pub mod numutil;
#[cfg(feature = "serde")]
pub mod serialize;
#[cfg(feature = "std")]
pub mod testing;

/// Maximum pointer size used by any system. This is used in some places, like
/// the debugger, to avoid needing to use generic parameters.
pub type Pointer = u32;

/// Colour type used by the system's PPUs for image data.
/// This type is analogus to egui's `Color32`, which allows the GUI to
/// simply `mem::transmute` it without having to perform any explicit
/// conversion. Additionally, due to this approach the core crate does not need
/// to depend on the rather heavy egui.
pub type Colour = [u8; 4];

pub trait Core: Any + Send + Sync {
    /// Try to create a core instance from the given game cart, if it is
    /// valid for this core's system.
    /// Can also create an instance with no cart loaded, if possible.
    /// The core is free to .take() the cart if it's valid; when returning
    /// None it should not touch it.
    fn try_new(cart: &mut Option<GameCart>, config: &SystemConfig) -> Option<Box<Self>>
    where
        Self: Sized;

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
    fn save_state(&mut self) -> Vec<u8> {
        Vec::new()
    }
    /// Load a state produced by [save_state].
    /// Will restore the current cartridge and debugger.
    fn load_state(&mut self, _state: &[u8]) {}

    /// Get the current system time.
    fn get_time(&self) -> Time;
    /// Returns the screen size for the current system.
    fn screen_size(&self) -> [usize; 2];
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
        Vec::new()
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

        let skip = self.c().options.speed_multiplier;
        let invert = self.c().options.invert_audio_samples;
        let volume = if skip == 1 {
            self.c().config.volume
        } else {
            self.c().config.volume_ff
        };

        self.c_mut()
            .audio_buffer
            .update_output_chunk_size(samples.len() / 2);
        while !self.c().audio_buffer.can_fill_buffer(skip) {
            if !self.c().debugger.running {
                samples.fill(0.0);
                return;
            }
            self.advance();
        }

        self.c_mut().audio_buffer.fill_buffer(samples, skip, volume);
        if invert {
            samples.reverse();
        }
    }
}

/// Unsafe, mutable Arc.
#[repr(transparent)]
pub struct UnsafeArc<T>(Arc<UnsafeCell<T>>);

impl<T> UnsafeArc<T> {
    pub fn new(t: T) -> UnsafeArc<T> {
        UnsafeArc(Arc::new(UnsafeCell::new(t)))
    }
}

impl<T> core::ops::Deref for UnsafeArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.0.get()) }
    }
}

impl<T> core::ops::DerefMut for UnsafeArc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.0.get()) }
    }
}

impl<T> Clone for UnsafeArc<T> {
    fn clone(&self) -> UnsafeArc<T> {
        UnsafeArc(self.0.clone())
    }
}

impl<T> Default for UnsafeArc<T>
where
    T: Default,
{
    fn default() -> UnsafeArc<T> {
        UnsafeArc::new(Default::default())
    }
}

#[cfg(feature = "serde")]
mod imp {
    impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for super::UnsafeArc<T> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(Self::new(T::deserialize(deserializer)?))
        }
    }

    impl<T: serde::Serialize> serde::Serialize for super::UnsafeArc<T> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let s: &T = &(*self);
            s.serialize(serializer)
        }
    }
}

unsafe impl<T: Send> Send for UnsafeArc<T> {}
unsafe impl<T: Sync> Sync for UnsafeArc<T> {}
