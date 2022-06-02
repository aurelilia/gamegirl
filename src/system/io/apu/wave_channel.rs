use super::ApuChannel;
use serde::Deserialize;
use serde::Serialize;

const VOLUME_SHIFT_TABLE: [u8; 4] = [4, 0, 1, 2];

#[derive(Default, Deserialize, Serialize)]
pub struct WaveChannel {
    volume: u8,
    volume_shift: u8,
    frequency: u16,

    buffer: [u8; 16],
    buffer_position: u8,
    buffer_position_just_clocked: bool,

    frequency_timer: u16,

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

    pub fn clock(&mut self) {
        // wave is clocked two times
        for _ in 0..2 {
            self.buffer_position_just_clocked = false;
            if self.frequency_timer == 0 {
                self.clock_position();
                self.buffer_position_just_clocked = true;

                // reload timer
                self.frequency_timer = 0x7FF - self.frequency;
            } else {
                self.frequency_timer -= 1;
            }
        }
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
            if !self.cgb && !self.buffer_position_just_clocked {
                return None;
            }

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

    fn trigger(&mut self) {
        // if its DMG and will clock next, meaning that it is reading buffer now,
        // then activate the wave-ram rewrite bug
        //
        // Some bytes from wave-ram are rewritten based on the current index
        if !self.cgb && self.frequency_timer == 0 {
            // get the next index that will be incremented to in the next clock
            let index = ((self.buffer_position + 1) & 0x1F) / 2;

            if index < 4 {
                self.buffer[0] = self.buffer[index as usize];
            } else {
                let four_bytes_align_start = ((index / 4) * 4) as usize;
                for i in 0..4 {
                    self.buffer[i] = self.buffer[four_bytes_align_start + i];
                }
            }
        }

        self.buffer_position = 0;
        // no idea why `3` works here, but with this tests pass and found it
        // in other emulators
        self.frequency_timer = 0x7FF - self.frequency + 3;
    }

    fn set_dac_enable(&mut self, enabled: bool) {
        self.dac_enable = enabled;
    }

    fn dac_enabled(&self) -> bool {
        self.dac_enable
    }
}
