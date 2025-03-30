use std::{collections::HashMap, fmt::Display, format, hash::Hash, string::String, vec::Vec};

use common::common::input;
use serde::{de::DeserializeOwned, Serialize};

pub trait Key: Copy + Display + Eq + Hash + DeserializeOwned + Serialize {
    fn is_escape(self) -> bool;
    fn default_map() -> HashMap<InputSource<Self>, InputAction>;
}

/// Input configuration struct.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Input<K: Key> {
    #[serde(bound(deserialize = "K: DeserializeOwned"))]
    mappings: HashMap<InputSource<K>, InputAction>,
    /// If `Some`: Interpret next input given to `key_triggered` as configuring
    /// the given action.
    #[serde(skip)]
    #[serde(default)]
    pub pending: Option<InputAction>,
}

impl<K: Key> Input<K> {
    /// Get an action to perform after a key got pressed.
    pub fn key_triggered(&mut self, src: InputSource<K>) -> Option<InputAction> {
        if let Some(pending) = self.pending.take() {
            self.set(src, pending);
            None
        } else {
            self.mappings.get(&src).copied()
        }
    }

    /// Set a mapping.
    pub fn set(&mut self, src: InputSource<K>, value: InputAction) {
        match src {
            InputSource::Key(k) if k.is_escape() => {
                // ESC: unset all mappings
                for k in self.key_for(value).collect::<Vec<_>>() {
                    self.mappings.remove(&k);
                }
            }
            src => {
                self.mappings.insert(src, value);
            }
        }
    }

    /// Get the key for a certain action.
    pub fn key_for(&mut self, action: InputAction) -> impl Iterator<Item = InputSource<K>> + '_ {
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
            mappings: K::default_map(),
            pending: None,
        }
    }
}

impl<K: Key> Default for Input<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// An action that is to be performed when the user hits a key.
/// Can be a button or a hotkey, the latter is stored
/// as an index into an array of functions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum InputSource<K: Key> {
    #[serde(bound(deserialize = "K: DeserializeOwned"))]
    Key(K),
    Button(gilrs::Button),
    Axis {
        axis: gilrs::Axis,
        is_neg: bool,
    },
}

impl<K: Key> Display for InputSource<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputSource::Key(k) => write!(f, "{k}"),
            InputSource::Button(b) => write!(f, "{b:?}"),
            InputSource::Axis { axis, is_neg } if *is_neg => write!(f, "{axis:?}-"),
            InputSource::Axis { axis, .. } => write!(f, "{axis:?}+"),
        }
    }
}

/// An action that is to be performed when the user hits a key.
/// Can be a button or a hotkey, the latter is stored
/// as an index into an array of functions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum InputAction {
    Button(input::Button),
    Hotkey(u8),
}
