// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{collections::HashMap, fmt::Display};

use common::common::input::{self, Button::*};
use eframe::egui::Key;
pub use file_dialog::File;
use InputAction::*;

pub mod file_dialog;
mod hotkeys;

pub use hotkeys::HOTKEYS;

/// Input configuration struct.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Input {
    mappings: HashMap<InputSource, InputAction>,
    #[serde(skip)]
    #[serde(default)]
    pub(crate) pending: Option<InputAction>,
}

impl Input {
    /// Get a mapping.
    pub fn get(&self, src: InputSource) -> Option<InputAction> {
        self.mappings.get(&src).copied()
    }

    /// Set a mapping.
    pub fn set(&mut self, src: InputSource, value: InputAction) {
        if src == InputSource::Key(Key::Escape) {
            // ESC: unset all mappings
            for k in self.key_for(value).collect::<Vec<_>>() {
                self.mappings.remove(&k);
            }
        } else {
            self.mappings.insert(src, value);
        }
    }

    /// Get the key for a certain action.
    pub fn key_for(&mut self, action: InputAction) -> impl Iterator<Item = InputSource> + '_ {
        self.mappings
            .iter()
            .filter(move |(_, v)| **v == action)
            .map(|(k, _)| *k)
    }

    /// Get the key for a certain action, formatted to a string.
    pub fn key_for_fmt(&mut self, action: InputAction) -> String {
        let mut keys = self
            .key_for(action)
            .map(|k| format!("{k}"))
            .collect::<Vec<_>>();
        keys.sort();
        keys.join(", ")
    }

    pub fn new() -> Self {
        Self {
            mappings: HashMap::from([
                (InputSource::Key(Key::X), Button(A)),
                (InputSource::Key(Key::Z), Button(B)),
                (InputSource::Key(Key::Enter), Button(Start)),
                (InputSource::Key(Key::Space), Button(Select)),
                (InputSource::Key(Key::ArrowDown), Button(Down)),
                (InputSource::Key(Key::ArrowUp), Button(Up)),
                (InputSource::Key(Key::ArrowLeft), Button(Left)),
                (InputSource::Key(Key::ArrowRight), Button(Right)),
                (InputSource::Key(Key::A), Button(L)),
                (InputSource::Key(Key::S), Button(R)),
                (InputSource::Key(Key::R), Hotkey(4)),
            ]),
            pending: None,
        }
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

/// An action that is to be performed when the user hits a key.
/// Can be a button or a hotkey, the latter is stored
/// as an index into an array of functions.
#[derive(Copy, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum InputSource {
    Key(Key),
    Button(gilrs::Button),
    Axis { axis: gilrs::Axis, is_neg: bool },
}

impl Display for InputSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputSource::Key(k) => write!(f, "{k:?}"),
            InputSource::Button(b) => write!(f, "{b:?}"),
            InputSource::Axis { axis, is_neg } if *is_neg => write!(f, "{axis:?}-"),
            InputSource::Axis { axis, .. } => write!(f, "{axis:?}+"),
        }
    }
}

/// An action that is to be performed when the user hits a key.
/// Can be a button or a hotkey, the latter is stored
/// as an index into an array of functions.
#[derive(Copy, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum InputAction {
    Button(input::Button),
    Hotkey(u8),
}
