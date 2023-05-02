// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod components;
pub mod misc;
#[macro_use]
pub mod macros;
pub mod numutil;

/// Audio sample rate of all emulated systems.
pub const SAMPLE_RATE: u32 = 44100;

/// For debugging: If instruction-level tracing output should be printed.
pub const TRACING: bool = false;

/// Colour type used by the system's PPUs for image data.
/// This type is analogus to egui's `Color32`, which allows the GUI to
/// simply `mem::transmute` it without having to perform any explicit
/// conversion. Additionally, due to this approach the core crate does not need
/// to depend on the rather heavy egui.
pub type Colour = [u8; 4];
