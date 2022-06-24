use serde::{Deserialize, Serialize};

use crate::{
    common::Button,
    ggc::{cpu::Interrupt, io::addr::JOYP, GameGirl},
};

/// Joypad of the console.
#[derive(Default, Deserialize, Serialize)]
pub struct Joypad {
    key_states: [bool; 8],
}

impl Joypad {
    pub fn read(&self, joyp: u8) -> u8 {
        let row_start = match joyp & 0x30 {
            0x10 => 0,
            0x20 => 4,
            _ => return 0xCF,
        };
        let mut res = 0;
        for key in self.key_states.iter().skip(row_start).take(4).rev() {
            res <<= 1;
            res += (!key) as u8;
        }
        res | (joyp & 0x30) | 0b1100_0000
    }

    /// To be called by GUI code; sets the state of a given button.
    pub fn set(gg: &mut GameGirl, button: Button, state: bool) {
        if button as usize >= 8 {
            return; // GGA buttons
        }
        gg.mmu.joypad.key_states[button as usize] = state;
        let read = gg.mmu.joypad.read(gg.mmu[JOYP]);
        if read & 0x0F != 0x0F {
            gg.request_interrupt(Interrupt::Joypad);
        }
    }
}
