// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{components::input::Button, numutil::NumExt};

use crate::Nes;

/// Joypad 1 of the console.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Joypad {
    key_states: [bool; 8],
    register: u8,
    strobe: bool,
}

impl Joypad {
    pub fn read(&mut self) -> u8 {
        let value = self.register & 1;
        self.register >>= 1;
        self.register.set_bit(7, true);
        value
    }

    pub fn write(&mut self, value: u8) {
        self.strobe = value.is_bit(0);
        if self.strobe {
            self.register = 0;
            self.register
                .set_bit(0, self.key_states[Button::A as usize]);
            self.register
                .set_bit(1, self.key_states[Button::B as usize]);
            self.register
                .set_bit(2, self.key_states[Button::Select as usize]);
            self.register
                .set_bit(3, self.key_states[Button::Start as usize]);
            self.register
                .set_bit(4, self.key_states[Button::Up as usize]);
            self.register
                .set_bit(5, self.key_states[Button::Down as usize]);
            self.register
                .set_bit(6, self.key_states[Button::Left as usize]);
            self.register
                .set_bit(7, self.key_states[Button::Right as usize]);
        }
    }

    /// To be called by GUI code; sets the state of a given button.
    pub fn set(nes: &mut Nes, button: Button, state: bool) {
        if button as usize >= 8 {
            return; // GGA buttons
        }
        nes.joypad.key_states[button as usize] = state;
    }
}
