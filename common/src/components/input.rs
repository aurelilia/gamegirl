// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

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
}

impl Button {
    pub const BUTTONS: [Self; 10] = [
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
    ];
}

/// The current state of buttons on a system.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ButtonState(pub u16);

impl ButtonState {
    fn set(self, button: Button, state: bool) -> Self {
        Self(self.0.set_bit(button as u16, state))
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Input {
    external: ButtonState,
    pub replay: ReplayState,
}

impl Input {
    pub fn state(&self, time: Time) -> ButtonState {
        if let ReplayState::Playback(ir) = &self.replay {
            ir.get_at(time)
        } else {
            self.external
        }
    }

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

    pub fn load_replay(&mut self, file: Vec<u8>) {
        self.replay = ReplayState::Playback(InputReplay::load(String::from_utf8(file).unwrap()));
    }

    pub fn new() -> Self {
        Self {
            external: ButtonState::default(),
            replay: ReplayState::None,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ReplayState {
    None,
    Recording(InputReplay),
    Playback(InputReplay),
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InputReplay {
    pub file: PathBuf,
    pub states: BTreeMap<Time, ButtonState>,
}

impl InputReplay {
    pub fn empty(file: PathBuf) -> Self {
        Self {
            file,
            states: BTreeMap::new(),
        }
    }

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

    pub fn add_state(&mut self, time: Time, state: ButtonState) {
        self.states.insert(time, state);
    }

    pub fn get_at(&self, time: Time) -> ButtonState {
        self.states
            .lower_bound(Bound::Excluded(&time))
            .peek_prev()
            .map(|(_, v)| *v)
            .unwrap_or_default()
    }

    pub fn to_string(&self) -> String {
        self.states.iter().fold(
            format!("{}\n", self.file.to_str().unwrap()),
            |mut acc, e| {
                writeln!(acc, "{:010b}|{}", e.1 .0, e.0).unwrap();
                acc
            },
        )
    }
}
