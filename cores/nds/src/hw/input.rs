// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! Input handler.

use armchair::{Cpu, Interrupt};
use common::TimeS;
use modular_bitfield::{bitfield, specifiers::B14};

use crate::{cpu::NDS9_CLOCK, scheduling::NdsEvent, CpuDevice, Nds, NdsCpu};

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Input {
    pub cnt: CpuDevice<KeyControl>,
    keys_prev: u16,
}

#[bitfield]
#[repr(u16)]
#[derive(Default, Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct KeyControl {
    irq_enables: B14,
    global_irq: bool,
    irq_is_and: bool,
}

impl Nds {
    pub fn keyinput(&self) -> u16 {
        // NDS input is active low
        0x3FF ^ (self.c.input.state(self.scheduler.now()).0 & 0x3FF)
    }

    pub fn keyinput_ext(&self) -> u16 {
        // NDS input is active low
        // TODO Touchscreen
        0b0111_1100 | (0x3 ^ (self.c.input.state(self.scheduler.now()).0 >> 10))
    }

    /// Check if KEYCNT should cause a joypad IRQ.
    pub fn check_keycnt<DS: NdsCpu>(ds: &mut DS) {
        let input = 0x3FF ^ ds.keyinput();
        let cnt = ds.input.cnt[DS::I];
        if cnt.global_irq() {
            let cond = cnt.irq_enables();
            let fire = (input != ds.input.keys_prev)
                && if !cnt.irq_is_and() {
                    cond & input != 0
                } else {
                    cond & input == cond
                };
            if fire {
                ds.cpu().request_interrupt(Interrupt::Joypad);
            }
        }

        ds.input.keys_prev = input;
    }
}
