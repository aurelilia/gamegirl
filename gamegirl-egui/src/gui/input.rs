// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::HashMap;

use common::misc::{Button, Button::*};
use eframe::egui::Key;
use InputAction::*;

use crate::gui::{file_dialog, App};

type HotkeyFn = fn(&mut App, bool);
pub(super) const HOTKEYS: &[(&str, HotkeyFn)] = &[
    ("Open ROM", |a, p| {
        pressed(a, p, |app| file_dialog::open(app.message_channel.0.clone()))
    }),
    ("Reset", |a, p| {
        pressed(a, p, |app| app.gg.lock().unwrap().reset())
    }),
    ("Pause", |a, p| {
        pressed(a, p, |app| {
            let mut gg = app.gg.lock().unwrap();
            gg.options().running = !gg.options().running && gg.options().rom_loaded;
        })
    }),
    ("Save", |a, p| pressed(a, p, |app| app.save_game())),
    ("Fast Forward (Hold)", |app, pressed| {
        let mut gg = app.gg.lock().unwrap();
        if pressed {
            gg.options().speed_multiplier = app.state.options.fast_forward_hold_speed;
        } else {
            gg.options().speed_multiplier = 1;
        }
    }),
    ("Fast Forward (Toggle)", |a, p| {
        pressed(a, p, |app| {
            let mut gg = app.gg.lock().unwrap();
            app.fast_forward_toggled = !app.fast_forward_toggled;
            if app.fast_forward_toggled {
                gg.options().speed_multiplier = app.state.options.fast_forward_toggle_speed;
            } else {
                gg.options().speed_multiplier = 1;
            }
        });
    }),
    #[cfg(feature = "savestates")]
    ("Rewind (Hold)", |app, pressed| {
        app.rewinder.rewinding = pressed;
        app.gg.lock().unwrap().options().invert_audio_samples = pressed;
    }),
];

fn pressed(app: &mut App, pressed: bool, inner: fn(&mut App)) {
    if pressed {
        inner(app);
    }
}

/// Input configuration struct.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct Input {
    mappings: HashMap<Key, InputAction>,
    #[cfg_attr(feature = "persistence", serde(skip))]
    #[cfg_attr(feature = "persistence", serde(default))]
    pub(crate) pending: Option<InputAction>,
}

impl Input {
    /// Get a key's mapping.
    pub fn get_key(&self, key: Key) -> Option<InputAction> {
        self.mappings.get(&key).copied()
    }

    /// Set a key's mapping.
    pub fn set_key(&mut self, key: Key, value: InputAction) {
        if let Some(prev) = self.key_for(value) {
            self.mappings.remove(&prev);
        }
        self.mappings.insert(key, value);
    }

    /// Get the key for a certain action.
    pub fn key_for(&mut self, action: InputAction) -> Option<Key> {
        self.mappings
            .iter()
            .find(|(_, v)| **v == action)
            .map(|(k, _)| *k)
    }

    /// Get the key for a certain action, formatted to a string.
    pub fn key_for_fmt(&mut self, action: InputAction) -> String {
        match self.key_for(action) {
            Some(key) => format!("{:?}", key),
            None => "<None>".to_string(),
        }
    }

    pub fn new() -> Self {
        Self {
            mappings: HashMap::from([
                (Key::X, Button(A)),
                (Key::Z, Button(B)),
                (Key::Enter, Button(Start)),
                (Key::Space, Button(Select)),
                (Key::ArrowDown, Button(Down)),
                (Key::ArrowUp, Button(Up)),
                (Key::ArrowLeft, Button(Left)),
                (Key::ArrowRight, Button(Right)),
                (Key::A, Button(L)),
                (Key::S, Button(R)),
                (Key::R, Hotkey(4)),
            ]),
            pending: None,
        }
    }
}

/// An action that is to be performed when the user hits a key.
/// Can be a button or a hotkey, the latter is stored
/// as an index into an array of functions.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub enum InputAction {
    Button(Button),
    Hotkey(u8),
}
