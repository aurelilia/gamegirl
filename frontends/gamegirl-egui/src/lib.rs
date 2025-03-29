// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![feature(ptr_metadata)]

mod app;
mod debug;
mod filter;
mod gui;
mod input;
mod rewind;

pub use app::App;
use eframe::egui::Color32;

/// Colour type used by the PPU for display output.
pub type Colour = Color32;
