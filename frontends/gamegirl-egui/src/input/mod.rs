// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{collections::HashMap, fmt::Display};

use eframe::egui;
pub use file_dialog::File;
use gamegirl::{
    common::common::input::Button::*,
    frontend::input::{
        InputAction::{self, *},
        InputSource, Key,
    },
};

pub mod file_dialog;
mod hotkeys;

pub use hotkeys::HOTKEYS;

#[derive(Copy, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EguiKey(egui::Key);

impl From<egui::Key> for EguiKey {
    fn from(value: egui::Key) -> Self {
        Self(value)
    }
}

impl Display for EguiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Key for EguiKey {
    fn is_escape(self) -> bool {
        self.0 == egui::Key::Escape
    }

    fn default_map() -> HashMap<gamegirl::frontend::input::InputSource<Self>, InputAction> {
        HashMap::from([
            (InputSource::Key(Self(egui::Key::X)), Button(A)),
            (InputSource::Key(Self(egui::Key::Z)), Button(B)),
            (InputSource::Key(Self(egui::Key::Enter)), Button(Start)),
            (InputSource::Key(Self(egui::Key::Space)), Button(Select)),
            (InputSource::Key(Self(egui::Key::ArrowDown)), Button(Down)),
            (InputSource::Key(Self(egui::Key::ArrowUp)), Button(Up)),
            (InputSource::Key(Self(egui::Key::ArrowLeft)), Button(Left)),
            (InputSource::Key(Self(egui::Key::ArrowRight)), Button(Right)),
            (InputSource::Key(Self(egui::Key::A)), Button(L)),
            (InputSource::Key(Self(egui::Key::S)), Button(R)),
            (InputSource::Key(Self(egui::Key::R)), Hotkey(4)),
        ])
    }
}
