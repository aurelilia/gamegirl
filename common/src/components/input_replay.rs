// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{fmt::Write, path::PathBuf};

use crate::misc::Button;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InputReplay {
    pub file: PathBuf,
    pub inputs: Vec<Input>,
    pub current: f64,
    pub current_input: usize,
}

impl InputReplay {
    pub fn new(str: String) -> Self {
        let mut lines = str.lines();
        let file = lines.next().unwrap().to_string().into();
        InputReplay {
            file,
            inputs: lines.map(Input::new).collect(),
            current: 0.0,
            current_input: 0,
        }
    }

    pub fn add_input(&mut self, input: Input) {
        self.inputs.push(input);
    }

    pub fn advance(&mut self, time: f64) -> Option<Input> {
        self.current += time;
        if self.inputs[self.current_input].time < self.current {
            self.current_input += 1;
            Some(self.inputs[self.current_input - 1])
        } else {
            None
        }
    }

    pub fn to_string(&self) -> String {
        self.inputs.iter().fold(
            format!("{}\n", self.file.to_str().unwrap()),
            |mut acc, e| {
                writeln!(acc, "{};{};{}", e.button as u32, e.state as u32, e.time).unwrap();
                acc
            },
        )
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Copy, Clone)]
pub struct Input {
    pub button: Button,
    pub state: bool,
    pub time: f64,
}

impl Input {
    pub fn new(str: &str) -> Self {
        let [button, state, time] = str.split(';').collect::<Vec<_>>()[..] else {
            panic!()
        };
        Self {
            button: Button::BUTTONS[button.parse::<usize>().unwrap()],
            state: state == "1",
            time: time.parse().unwrap(),
        }
    }
}
