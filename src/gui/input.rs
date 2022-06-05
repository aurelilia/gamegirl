use crate::system::io::joypad::Button;
use crate::system::io::joypad::Button::*;
use eframe::egui::Key;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use InputAction::*;

/// Input configuration struct.
#[derive(Deserialize, Serialize)]
pub struct Input {
    mappings: HashMap<Key, InputAction>,
}

impl Input {
    /// Get a key's mapping.
    pub fn get_key(&self, key: Key) -> Option<InputAction> {
        self.mappings.get(&key).copied()
    }

    /// Set a key's mapping.
    pub fn set_key(&mut self, key: Key, value: InputAction) {
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
            ]),
        }
    }
}

/// An action that is to be performed when the user hits a key.
/// Can be a button or a hotkey, the latter is stored
/// as an index into an array of functions.
#[derive(Copy, Clone, PartialEq, Hash, Deserialize, Serialize)]
pub enum InputAction {
    Button(Button),
    Hotkey(u8),
}
