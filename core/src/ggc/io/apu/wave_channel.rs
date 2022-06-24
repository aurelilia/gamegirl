use super::{ApuChannel, ScheduleFn};
use crate::{ggc::io::apu::GenApuEvent, numutil::NumExt};
use serde::{Deserialize, Serialize};

const VOLUME_SHIFT_TABLE: [u8; 4] = [4, 0, 1, 2];

#[derive(Default, Deserialize, Serialize)]
pub struct WaveChannel {
    volume: u8,
    volume_shift: u8,
    frequency: u16,

    buffer: [u8; 16],
    buffer_position: u8,

    channel_enable: bool,
    dac_enable: bool,

    cgb: bool,
}

impl WaveChannel {
    pub fn new(cgb: bool) -> Self {
        Self {
            cgb,
            ..Self::default()
        }
    }

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
        if let Some(index) = self.wave_buffer_index(offset) {
            self.buffer[index] = data;
        }
    }

    pub fn read_buffer(&self, offset: u8) -> u8 {
        if let Some(index) = self.wave_buffer_index(offset) {
            self.buffer[index]
        } else {
            0xFF
        }
    }

    pub fn clock(&mut self) -> u32 {
        self.clock_position();
        (0x7FF - self.frequency).u32() << 2
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
    fn wave_buffer_index(&self, offset: u8) -> Option<usize> {
        let index = if self.dac_enable && self.channel_enable {
            self.buffer_position / 2
        } else {
            offset
        } as usize
            & 0xF;

        Some(index)
    }
}

impl ApuChannel for WaveChannel {
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
            (0x7FF - self.frequency + 3).u32() << 2,
        );
    }

    fn set_dac_enable(&mut self, enabled: bool) {
        self.dac_enable = enabled;
    }

    fn dac_enabled(&self) -> bool {
        self.dac_enable
    }
}
