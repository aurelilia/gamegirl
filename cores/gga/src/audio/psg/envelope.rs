// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EnvelopGenerator {
    starting_volume: u8,
    current_volume: u8,
    sweep_increase: bool,
    period: u8,

    envelope_can_run: bool,

    counter: u8,
}

impl EnvelopGenerator {
    pub fn write_envelope_register(&mut self, data: u8) {
        // TODO: is initial volume different?
        self.starting_volume = data >> 4;
        self.current_volume = self.starting_volume;
        self.sweep_increase = (data >> 3) & 1 == 1;
        self.period = data & 7;
        self.counter = self.period;
    }

    pub fn read_envelope_register(&self) -> u8 {
        ((self.starting_volume & 0xF) << 4) | ((self.sweep_increase as u8) << 3) | (self.period & 7)
    }

    pub fn current_volume(&self) -> u8 {
        self.current_volume
    }

    pub fn clock(&mut self) {
        self.counter = self.counter.saturating_sub(1);

        if self.counter == 0 {
            self.counter = self.period;
            if self.counter == 0 {
                self.counter = 8;
            }

            if self.envelope_can_run && self.period != 0 {
                if self.sweep_increase {
                    if self.current_volume < 15 {
                        self.current_volume += 1;
                    }
                } else {
                    self.current_volume = self.current_volume.saturating_sub(1);
                }

                if self.current_volume == 0 || self.current_volume == 15 {
                    self.envelope_can_run = false;
                }
            }
        }
    }

    pub fn trigger(&mut self) {
        self.counter = self.period;
        self.current_volume = self.starting_volume;
        self.envelope_can_run = true;
    }
}
