// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::numutil::NumExt;

use crate::{cpu::Interrupt, io::addr::*, GameGirl};

/// Timer available on DMG and CGB.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Timer {
    system_counter: u16,
    tima_just_overflowed: bool,
    tima_just_written: bool,
}

impl Timer {
    pub fn step(gg: &mut GameGirl) {
        if gg.timer.tima_just_overflowed {
            gg.request_interrupt(Interrupt::Timer);
            gg[TIMA] = gg[TMA];
        }
        gg.timer.tima_just_overflowed = false;

        Self::change_counter(gg, gg.timer.system_counter.wrapping_add(1));
        gg.timer.tima_just_written = false;
    }

    fn change_counter(gg: &mut GameGirl, new: u16) {
        let old = gg.timer.system_counter;
        gg.timer.system_counter = new;
        if Self::did_edge_fall(old, new, Self::tac_index(gg)) {
            Self::tick_timer(gg);
        }
    }

    fn did_edge_fall<T: NumExt>(old: T, new: T, bit: u16) -> bool {
        old.is_bit(bit) && !new.is_bit(bit)
    }

    fn get_tac_bit(gg: &mut GameGirl) -> bool {
        gg.timer.system_counter.is_bit(Self::tac_index(gg))
    }

    fn tac_index(gg: &GameGirl) -> u16 {
        [7, 1, 3, 5][(gg[TAC] & 3).us()]
    }

    fn tick_timer(gg: &mut GameGirl) {
        if gg[TAC].is_bit(2) {
            let old = gg[TIMA];
            gg[TIMA] = gg[TIMA].wrapping_add(1);
            gg.timer.tima_just_overflowed =
                Self::did_edge_fall(old, gg[TIMA], 7) && !gg.timer.tima_just_written;
        }
    }

    pub fn read(gg: &GameGirl, addr: u16) -> u8 {
        match addr {
            DIV => (gg.timer.system_counter >> 6).u8(),
            _ => 0xFF,
        }
    }

    pub fn write(gg: &mut GameGirl, addr: u16, value: u8) {
        match addr {
            DIV => Self::change_counter(gg, 0),
            TIMA => {
                gg.timer.tima_just_written = true;
                gg[TIMA] = value;
            }
            TMA => gg[TMA] = value,
            TAC => {
                let old_tac = gg[TAC];
                let old_select = Self::get_tac_bit(gg);
                let was_on = old_tac.is_bit(2);

                gg[TAC] = value | 0xF8;
                let new_select = Self::get_tac_bit(gg);
                let is_on = value.is_bit(2);

                let before = was_on && old_select;
                let now = is_on && new_select;

                if before && !now {
                    Self::tick_timer(gg);
                }
            }
            _ => (),
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            system_counter: 4889, // Exact value required to get mooneye to pass.
            tima_just_overflowed: false,
            tima_just_written: false,
        }
    }
}
