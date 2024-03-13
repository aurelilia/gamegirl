use std::{fmt::Write, path::PathBuf};

use common::misc::Button;

pub struct InputReplay {
    pub file: PathBuf,
    pub inputs: Vec<Input>,
    pub current: f64,
}

impl InputReplay {
    pub fn new(str: String) -> Self {
        let mut lines = str.lines();
        let file = lines.next().unwrap().to_string().into();
        InputReplay {
            file,
            inputs: lines.map(Input::new).collect(),
            current: 0.0,
        }
    }

    pub fn add_input(&mut self, input: Input) {
        self.inputs.push(input);
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
