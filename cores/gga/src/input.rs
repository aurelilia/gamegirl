// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! Input handler.
//! Luckily, GGA input is dead simple compared to even GG.

use arm_cpu::{Cpu, Interrupt};
use modular_bitfield::{bitfield, specifiers::B14};

use crate::GameGirlAdv;

#[bitfield]
#[repr(u16)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct KeyControl {
    irq_enables: B14,
    global_irq: bool,
    irq_is_and: bool,
}

impl GameGirlAdv {
    pub fn keyinput(&self) -> u16 {
        // GGA input is active low
        0x3FF ^ self.options.input.state(self.scheduler.now()).0
    }

    /// Check if KEYCNT should cause a joypad IRQ.
    pub fn check_keycnt(&mut self) {
        let input = self.keyinput();
        let cnt = self.memory.keycnt;
        if cnt.global_irq() {
            let cond = cnt.irq_enables();
            let fire = if !cnt.irq_is_and() {
                cond & input != 0
            } else {
                cond & input == cond
            };
            if fire {
                Cpu::request_interrupt(self, Interrupt::Joypad);
            }
        }
    }
}
