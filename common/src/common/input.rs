// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{collections::BTreeMap, fmt::Write, ops::Bound, path::PathBuf};

use crate::{numutil::NumExt, Time};

/// Buttons on a system. Not all are used for all systems.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde_config", derive(serde::Deserialize, serde::Serialize))]
#[repr(C)]
pub enum Button {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
    R = 8,
    L = 9,
    X = 10,
    Y = 11,
}

impl Button {
    pub const BUTTONS: [Self; 12] = [
        Self::A,
        Self::B,
        Self::Select,
        Self::Start,
        Self::Right,
        Self::Left,
        Self::Up,
        Self::Down,
        Self::R,
        Self::L,
        Self::X,
        Self::Y,
    ];
}

/// The current state of buttons on a system.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ButtonState(pub u16);

impl ButtonState {
    /// Set the state of the given button.
    fn set(self, button: Button, state: bool) -> Self {
        Self(self.0.set_bit(button as u16, state))
    }
}

/// Input subsystem to be used by emulation cores.
/// Contains external input and replay state, if a replay is loaded.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Input {
    external: ButtonState,
    pub replay: ReplayState,
}

impl Input {
    /// Get the button state at the given time in system ticks.
    pub fn state(&self, time: Time) -> ButtonState {
        if let ReplayState::Playback(ir) = &self.replay {
            ir.get_at(time)
        } else {
            self.external
        }
    }

    /// Set the state of a button at the given time in system ticks.
    pub fn set(&mut self, time: Time, button: Button, state: bool) {
        let new = self.external.set(button, state);
        if self.external != new {
            self.external = new;
            let ReplayState::Recording(ir) = &mut self.replay else {
                return;
            };
            ir.add_state(time, self.external);
        }
    }

    /// Load a replay from a raw file.
    pub fn load_replay(&mut self, file: Vec<u8>) {
        self.replay = ReplayState::Playback(InputReplay::load(String::from_utf8(file).unwrap()));
    }
}

/// State of replay, while a core is running.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ReplayState {
    /// No replay loaded.
    #[default]
    None,
    /// Recording a new replay.
    Recording(InputReplay),
    /// Playing back a loaded replay.
    Playback(InputReplay),
}

/// An input replay that can be loaded and stored in .rpl files.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InputReplay {
    /// File name of the replay.
    pub file: PathBuf,
    /// A list of button states at given times.
    pub states: BTreeMap<Time, ButtonState>,
}

impl InputReplay {
    /// Create a new empty replay with the given file name.
    pub fn empty(file: PathBuf) -> Self {
        Self {
            file,
            states: BTreeMap::new(),
        }
    }

    /// Load a replay from a string, in .rpl format.
    pub fn load(str: String) -> Self {
        let mut lines = str.lines();
        let file = lines.next().unwrap().to_string().into();
        InputReplay {
            file,
            states: lines
                .map(|l| {
                    let (buttons, time) = l.split_once("|").unwrap();
                    (
                        time.parse().unwrap(),
                        ButtonState(u16::from_str_radix(buttons, 2).unwrap()),
                    )
                })
                .collect(),
        }
    }

    /// Add a new button state at the given time.
    /// Used when recording new replays.
    pub fn add_state(&mut self, time: Time, state: ButtonState) {
        self.states.insert(time, state);
    }

    /// Get the button state for the given time.
    pub fn get_at(&self, time: Time) -> ButtonState {
        self.states
            .lower_bound(Bound::Excluded(&time))
            .peek_prev()
            .map(|(_, v)| *v)
            .unwrap_or_default()
    }

    /// Save the replay to a string, in .rpl format.
    pub fn serialize(&self) -> String {
        self.states.iter().fold(
            format!("{}\n", self.file.to_str().unwrap()),
            |mut acc, e| {
                writeln!(acc, "{:010b}|{}", e.1 .0, e.0).unwrap();
                acc
            },
        )
    }
}
