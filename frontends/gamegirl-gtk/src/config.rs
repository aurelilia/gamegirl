use std::path::PathBuf;

use gamegirl::{
    SystemConfig,
    frontend::{input::Input, rewinder::RewinderConfig},
};

use crate::gui::input::GtkKey;

/// State that is persisted on app reboot.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct State {
    /// A list of last opened ROMs. Size is capped to 10, last opened
    /// ROM is at index 0. The oldest ROM gets removed first.
    pub last_opened: Vec<PathBuf>,
    /// User configuration options.
    pub options: Options,
}

/// User-configurable options.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Options {
    /// Options passed to the system when loading a ROM.
    pub sys: SystemConfig,
    /// Input configuration.
    pub input: Input<GtkKey>,
    /// Rewinder configuration.
    pub rewinder: RewinderConfig,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sys: Default::default(),
            input: Input::new(),
            rewinder: RewinderConfig::default(),
        }
    }
}
