// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{numutil::NumExt, TimeS};

use super::scheduling::GGEvent;
use crate::{cpu::Interrupt, io::addr::JOYP, GameGirl, T_CLOCK_HZ};

/// Joypad of the console.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Joypad {
    key_states: u8,
}

impl Joypad {
    pub fn read(&self, joyp: u8) -> u8 {
        let row_start = match joyp & 0x30 {
            0x10 => 0,
            0x20 => 4,
            _ => return 0xCF,
        };
        let mut res = 0;
        for key in (0..8).skip(row_start).take(4).rev() {
            let key = self.key_states.is_bit(key);
            res <<= 1;
            res += (!key) as u8;
        }
        res | (joyp & 0x30) | 0b1100_0000
    }

    pub fn update(gg: &mut GameGirl) {
        gg.joypad.key_states = gg.options.input.state(gg.scheduler.now()).0 as u8;
        gg.scheduler
            .schedule(GGEvent::UpdateKeypad, (T_CLOCK_HZ / 120) as TimeS);
        let read = gg.joypad.read(gg[JOYP]);
        if read & 0x0F != 0x0F {
            gg.request_interrupt(Interrupt::Joypad);
        }
    }
}
