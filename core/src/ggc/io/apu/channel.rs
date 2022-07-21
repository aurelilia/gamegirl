// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use super::ScheduleFn;

pub trait Channel {
    fn output(&self) -> u8;
    fn muted(&self) -> bool;
    fn set_enable(&mut self, enabled: bool);
    fn enabled(&self) -> bool;
    fn set_dac_enable(&mut self, enabled: bool);
    fn dac_enabled(&self) -> bool;
    fn trigger(&mut self, sched: &mut impl ScheduleFn);
}

#[derive(Deserialize, Serialize)]
pub struct LengthCountedChannel<C: Channel> {
    // FIXME: re-order the organization of apu channels,
    //  `dac_enable`, should not be here like this
    // dac_enable: bool,
    max_length: u16,
    length: u16,
    current_counter: u16,
    counter_decrease_enable: bool,
    channel: C,
}

impl<C: Channel> LengthCountedChannel<C> {
    pub fn new(channel: C, max_length: u16) -> Self {
        Self {
            // dac_enable: true,
            max_length,
            length: 0,
            current_counter: 0,
            counter_decrease_enable: false,
            channel,
        }
    }
    pub fn channel(&self) -> &C {
        &self.channel
    }

    pub fn channel_mut(&mut self) -> &mut C {
        &mut self.channel
    }

    pub fn write_sound_length(&mut self, data: u8) {
        self.length = self.max_length - data as u16;
        self.current_counter = self.length;
    }

    pub fn write_length_enable(&mut self, data: bool) {
        self.counter_decrease_enable = data;
    }

    pub fn read_length_enable(&self) -> bool {
        self.counter_decrease_enable
    }

    pub fn clock_length_counter(&mut self) {
        if self.counter_decrease_enable {
            if self.current_counter == 0 {
                self.set_enable(false);
            } else {
                self.current_counter -= 1;
                if self.current_counter == 0 {
                    self.set_enable(false);
                }
            }
        }
    }

    // The code needs imporovements :(
    //
    /// A wrapper around `trigger` to account for the special case where the
    /// current counter is clocked after reloading to the max length
    pub fn trigger_length(&mut self, is_not_length_clock_next: bool, sched: &mut impl ScheduleFn) {
        if self.current_counter == 0 {
            self.current_counter =
                self.max_length - (is_not_length_clock_next && self.counter_decrease_enable) as u16;
        }
        self.trigger(sched);
    }

    pub fn reset_length_counter(&mut self) {
        self.length = self.max_length;
        self.current_counter = self.length;
    }
}

impl<C: Channel> Channel for LengthCountedChannel<C> {
    fn output(&self) -> u8 {
        if !self.channel.enabled() {
            0
        } else {
            self.channel.output()
        }
    }

    fn muted(&self) -> bool {
        !self.channel.enabled() || self.channel.muted()
    }

    fn trigger(&mut self, sched: &mut impl ScheduleFn) {
        if self.dac_enabled() {
            self.set_enable(true);
        }

        self.channel.trigger(sched);
    }

    fn set_enable(&mut self, enabled: bool) {
        self.channel.set_enable(enabled);
    }

    fn enabled(&self) -> bool {
        self.channel.enabled()
    }

    fn set_dac_enable(&mut self, enabled: bool) {
        self.channel.set_dac_enable(enabled);

        if !enabled {
            self.set_enable(false);
        }
    }

    fn dac_enabled(&self) -> bool {
        self.channel.dac_enabled()
    }
}

#[derive(Deserialize, Serialize)]
pub struct Dac<C: Channel> {
    capacitor: f32,
    channel: C,
}

impl<C: Channel> Dac<C> {
    pub fn new(channel: C) -> Self {
        Self {
            capacitor: 0.,
            channel,
        }
    }

    pub fn dac_output(&mut self) -> f32 {
        if self.channel.muted() {
            0.
        } else {
            let dac_in = self.channel.output() as f32 / 15.;
            let dac_out = dac_in - self.capacitor;

            self.capacitor = dac_in - dac_out * 0.996;

            dac_out
        }
    }
}

impl<C: Channel> std::ops::Deref for Dac<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.channel
    }
}

impl<C: Channel> std::ops::DerefMut for Dac<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.channel
    }
}
