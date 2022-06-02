use super::envelope::EnvelopGenerator;
use super::ApuChannel;
use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Deserialize, Serialize)]
pub struct NoiseChannel {
    shift_clock_frequency: u8,
    step_mode_7_bits: bool,
    divisor_code: u8,

    frequency_timer: u16,

    feedback_shift_register: u16,

    envelope: EnvelopGenerator,

    channel_enabled: bool,

    dac_enable: bool,
}

impl NoiseChannel {
    pub fn write_noise_register(&mut self, data: u8) {
        self.shift_clock_frequency = data >> 4;
        self.step_mode_7_bits = (data >> 3) & 1 == 1;
        self.divisor_code = data & 7;
    }

    pub fn read_noise_register(&self) -> u8 {
        (self.shift_clock_frequency << 4) | ((self.step_mode_7_bits as u8) << 3) | self.divisor_code
    }

    pub fn envelope(&self) -> &EnvelopGenerator {
        &self.envelope
    }

    pub fn envelope_mut(&mut self) -> &mut EnvelopGenerator {
        &mut self.envelope
    }

    pub fn clock(&mut self) {
        if self.frequency_timer == 0 {
            self.clock_feedback_register();

            // reload timer
            self.frequency_timer = self.get_frequency();
        } else {
            self.frequency_timer -= 1;
        }
    }
}

impl NoiseChannel {
    fn get_frequency(&self) -> u16 {
        (self.base_divisor() << self.shift_clock_frequency) / 4
    }

    fn base_divisor(&self) -> u16 {
        if self.divisor_code == 0 {
            8
        } else {
            self.divisor_code as u16 * 16
        }
    }

    fn clock_feedback_register(&mut self) {
        let xor_result =
            (self.feedback_shift_register & 1) ^ ((self.feedback_shift_register >> 1) & 1);

        self.feedback_shift_register >>= 1;

        self.feedback_shift_register |= xor_result << 14;

        if self.step_mode_7_bits {
            self.feedback_shift_register &= !0x40;
            self.feedback_shift_register |= xor_result << 6;
        }
    }
}

impl ApuChannel for NoiseChannel {
    fn output(&self) -> u8 {
        ((self.feedback_shift_register & 1) ^ 1) as u8 * self.envelope.current_volume()
    }

    fn muted(&self) -> bool {
        self.envelope.current_volume() == 0
    }

    fn trigger(&mut self) {
        self.envelope.trigger();
        // because its 15 bits
        self.feedback_shift_register = 0x7FFF;
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
