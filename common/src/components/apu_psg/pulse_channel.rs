// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{envelope::EnvelopGenerator, Channel, GenApuEvent, ScheduleFn};
use crate::numutil::NumExt;

const DUTY_CYCLE_SEQUENCES: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PulseChannel<const SECOND: bool> {
    sweep_period: u8,
    sweep_current_time: u8,
    sweep_internal_enable: bool,
    sweep_frequency_shadow: u16,
    sweep_negate: bool,
    sweep_shift_n: u8,

    /// has sweep calculation happened at least once since last trigger
    sweep_calculation_happened: bool,

    sequencer_data: [u8; 8],
    sequencer_position: usize,
    duty: u8,
    envelope: EnvelopGenerator,
    frequency: u16,

    channel_enabled: bool,
    dac_enable: bool,
}

impl<const T: bool> Default for PulseChannel<T> {
    fn default() -> Self {
        Self {
            sweep_internal_enable: false,
            sweep_frequency_shadow: 0,
            sweep_period: 0,
            sweep_current_time: 0,
            sweep_negate: false,
            sweep_shift_n: 0,
            sweep_calculation_happened: false,
            duty: 0,
            sequencer_data: DUTY_CYCLE_SEQUENCES[0],
            sequencer_position: 0,
            envelope: EnvelopGenerator::default(),
            frequency: 0,
            channel_enabled: false,
            dac_enable: false,
        }
    }
}

impl<const T: bool> PulseChannel<T> {
    pub fn write_sweep_register(&mut self, data: u8) {
        let old_negate = self.sweep_negate;

        self.sweep_period = (data >> 4) & 7;
        self.sweep_negate = (data >> 3) & 1 == 1;
        self.sweep_shift_n = data & 7;

        // obscure behaviour: Clearing the sweep negate mode bit in NR10 after
        // at least one sweep calculation has been made using the negate mode
        // since the last trigger causes the channel to be immediately disabled
        if old_negate && !self.sweep_negate && self.sweep_calculation_happened {
            self.channel_enabled = false;
        }

        self.sweep_calculation_happened = false;
    }

    pub fn read_sweep_register(&self) -> u8 {
        ((self.sweep_period & 7) << 4) | ((self.sweep_negate as u8) << 3) | (self.sweep_shift_n & 7)
    }

    pub fn write_pattern_duty(&mut self, data: u8) {
        self.sequencer_data = DUTY_CYCLE_SEQUENCES[data as usize & 3];
        self.duty = data & 3;
    }

    pub fn read_pattern_duty(&self) -> u8 {
        self.duty & 3
    }

    pub fn frequency(&self) -> u16 {
        self.frequency
    }

    pub fn write_frequency(&mut self, data: u16) {
        self.frequency = data;
    }

    pub fn envelope(&self) -> &EnvelopGenerator {
        &self.envelope
    }

    pub fn envelope_mut(&mut self) -> &mut EnvelopGenerator {
        &mut self.envelope
    }

    pub fn clock(&mut self) -> u32 {
        self.clock_sequencer();
        (0x7FF - self.frequency).u32() << 2
    }

    pub fn clock_sweeper(&mut self) {
        self.sweep_current_time = self.sweep_current_time.saturating_sub(1);

        if self.sweep_current_time == 0 {
            self.reload_sweep_counter();

            if self.sweep_internal_enable && self.sweep_period != 0 {
                let new_freq = self.sweep_calculation();

                if new_freq <= 2047 && self.sweep_shift_n != 0 {
                    self.frequency = new_freq;
                    self.sweep_frequency_shadow = new_freq;
                    self.sweep_calculation();
                }
            }
        }
    }

    pub fn reset_sequencer(&mut self) {
        self.sequencer_position = 0;
    }
}

impl<const T: bool> PulseChannel<T> {
    fn clock_sequencer(&mut self) {
        self.sequencer_position = (self.sequencer_position + 1) & 7;
    }

    fn reload_sweep_counter(&mut self) {
        self.sweep_current_time = self.sweep_period;

        if self.sweep_current_time == 0 {
            self.sweep_current_time = 8;
        }
    }

    fn sweep_trigger(&mut self) {
        self.sweep_frequency_shadow = self.frequency;
        self.reload_sweep_counter();
        self.sweep_internal_enable = self.sweep_period != 0 || self.sweep_shift_n != 0;
        self.sweep_calculation_happened = false;

        if self.sweep_shift_n != 0 {
            self.sweep_calculation();
        }
    }

    fn sweep_calculation(&mut self) -> u16 {
        self.sweep_calculation_happened = true;

        let shifted_freq = self.sweep_frequency_shadow >> self.sweep_shift_n;

        let new_freq = if self.sweep_negate {
            self.sweep_frequency_shadow.wrapping_sub(shifted_freq)
        } else {
            self.sweep_frequency_shadow.wrapping_add(shifted_freq)
        };

        if new_freq > 2047 {
            self.channel_enabled = false;
        }

        new_freq
    }
}

impl<const T: bool> Channel for PulseChannel<T> {
    fn output(&self) -> u8 {
        self.sequencer_data[self.sequencer_position] * self.envelope.current_volume()
    }

    fn muted(&self) -> bool {
        self.envelope.current_volume() == 0
    }

    fn trigger(&mut self, sched: &mut impl ScheduleFn) {
        let evt = if T {
            GenApuEvent::Pulse2Reload
        } else {
            GenApuEvent::Pulse1Reload
        };
        sched(evt, ((0x7FF - self.frequency) as i32) << 2);
        self.envelope.trigger();
        self.sweep_trigger();
    }

    fn set_enable(&mut self, enabled: bool) {
        self.channel_enabled = enabled;
    }

    fn enabled(&self) -> bool {
        self.channel_enabled
    }

    fn set_dac_enable(&mut self, enabled: bool) {
        self.dac_enable = enabled;
    }

    fn dac_enabled(&self) -> bool {
        self.dac_enable
    }
}
