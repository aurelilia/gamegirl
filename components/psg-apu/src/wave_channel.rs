// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use super::{Channel, ScheduleFn};
use crate::{GenApuEvent, TimeS};

const VOLUME_SHIFT_TABLE: [u8; 4] = [4, 0, 1, 2];

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct WaveChannel {
    volume: u8,
    volume_shift: u8,
    frequency: u16,

    buffer: [u8; 16],
    buffer_position: u8,

    channel_enable: bool,
    dac_enable: bool,
}

impl WaveChannel {
    pub fn write_volume(&mut self, vol: u8) {
        self.volume = vol;
        self.volume_shift = VOLUME_SHIFT_TABLE[vol as usize & 3];
    }

    pub fn read_volume(&self) -> u8 {
        self.volume
    }

    pub fn frequency(&self) -> u16 {
        self.frequency
    }

    pub fn write_frequency(&mut self, data: u16) {
        self.frequency = data;
    }

    pub fn write_buffer(&mut self, offset: u8, data: u8) {
        self.buffer[self.wave_buffer_index(offset)] = data;
    }

    pub fn read_buffer(&self, offset: u8) -> u8 {
        self.buffer[self.wave_buffer_index(offset)]
    }

    pub fn clock(&mut self) -> u32 {
        self.clock_position();
        (0x7FF - self.frequency as u32) << 2
    }

    pub fn reset_buffer_index(&mut self) {
        self.buffer_position = 0;
    }
}

impl WaveChannel {
    fn clock_position(&mut self) {
        self.buffer_position = (self.buffer_position + 1) & 0x1F;
    }

    /// returns `Some` if the wave is accessable, `None` otherwise (for DMG)
    fn wave_buffer_index(&self, offset: u8) -> usize {
        (if self.dac_enable && self.channel_enable {
            self.buffer_position / 2
        } else {
            offset
        }) as usize
            & 0xF
    }
}

impl Channel for WaveChannel {
    fn output(&self) -> u8 {
        let byte = self.buffer[self.buffer_position as usize / 2];
        // the shift will be 4 if buffer_position is even, and 0 if its odd
        let shift = 4 * ((self.buffer_position & 1) ^ 1);
        let byte = (byte >> shift) & 0xF;

        byte >> self.volume_shift
    }

    fn muted(&self) -> bool {
        false
    }

    fn set_enable(&mut self, enabled: bool) {
        self.channel_enable = enabled;
    }

    fn enabled(&self) -> bool {
        self.channel_enable
    }

    fn trigger(&mut self, sched: &mut impl ScheduleFn) {
        self.buffer_position = 0;
        // no idea why `3` works here, but with this tests pass and found it
        // in other emulators
        sched(
            GenApuEvent::WaveReload,
            ((0x7FF - self.frequency + 3) as TimeS) << 2,
        );
    }

    fn set_dac_enable(&mut self, enabled: bool) {
        self.dac_enable = enabled;
    }

    fn dac_enabled(&self) -> bool {
        self.dac_enable
    }
}
