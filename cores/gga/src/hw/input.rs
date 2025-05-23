// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! Input handler.
//! Luckily, GGA input is dead simple compared to even GG.

#![allow(unused_braces)] // modular_bitfield issue

use armchair::Interrupt;
use common::TimeS;
use modular_bitfield::{bitfield, specifiers::B14};

use crate::{
    cpu::{GgaFullBus, CPU_CLOCK},
    scheduling::AdvEvent,
};

#[bitfield]
#[repr(u16)]
#[derive(Default, Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct KeyControl {
    irq_enables: B14,
    global_irq: bool,
    irq_is_and: bool,
}

impl GgaFullBus {
    pub fn keyinput(&self) -> u16 {
        // GGA input is active low
        0x3FF ^ self.c.input.state(self.scheduler.now()).0
    }

    /// Check if KEYCNT should cause a joypad IRQ.
    pub fn check_keycnt(&mut self) {
        let input = 0x3FF ^ self.keyinput();
        let cnt = self.memory.keycnt;
        if cnt.global_irq() {
            let cond = cnt.irq_enables();
            let fire = (input != self.memory.keys_prev)
                && if !cnt.irq_is_and() {
                    cond & input != 0
                } else {
                    cond & input == cond
                };
            if fire {
                self.cpu.request_interrupt(&mut self.bus, Interrupt::Joypad);
            }

            self.scheduler
                .schedule(AdvEvent::UpdateKeypad, (CPU_CLOCK / 120.0) as TimeS);
        }

        self.memory.keys_prev = input;
    }
}
