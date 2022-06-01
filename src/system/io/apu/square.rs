use crate::numutil::NumExt;
use crate::system::T_CLOCK_HZ;
use std::mem;

pub const DUTY_CYCLES: [u8; 4] = [0b00000001, 0b10000001, 0b10000111, 0b01111110];
const FR_DIVIDER: u32 = (T_CLOCK_HZ / 128) as u32;

pub struct SquareCh {
    pub duty: u8,
    pub duty_counter: u16,
    pub off: bool,
    pub timer: i16,
}

#[derive(Default)]
pub struct FreqSweep {
    timer: u8,
    enabled: bool,
    shift: u8,
    shadow_reg: u16,
    pub(crate) freq: u16,
    period: u8,
    negate: bool,
    counter: u32,
    calc_made: bool,
}

impl FreqSweep {
    // ret: if sound channel should be disabled.
    #[must_use]
    pub fn cycle(&mut self, cycles: u16) -> bool {
        self.counter += cycles as u32;
        if self.counter > FR_DIVIDER {
            self.counter -= FR_DIVIDER;
            if !self.enabled {
                return false;
            }

            self.timer = self.timer.wrapping_sub(1);
            if self.timer == 0 {
                self.timer = if self.period == 0 { 8 } else { self.period };

                if self.period != 0 {
                    let (freq, ret) = self.calculate();
                    if self.enabled && self.shift != 0 {
                        self.freq = freq;
                        self.shadow_reg = freq;
                        let (_, retb) = self.calculate();
                        return ret || retb;
                    }
                    return ret;
                };
            }
        }
        false
    }

    // bool ret: if sound channel should be disabled.
    fn calculate(&mut self) -> (u16, bool) {
        self.calc_made = true;
        let mut freq = self.shadow_reg >> self.shift;
        freq = if self.negate {
            self.shadow_reg - freq
        } else {
            self.shadow_reg + freq
        };

        self.enabled &= freq < 2048;
        (freq.min(2048), freq >= 2048)
    }

    // ret: if sound channel should be disabled.
    #[must_use]
    pub fn trigger(&mut self) -> bool {
        self.shadow_reg = self.freq;
        self.timer = if self.period == 0 { 8 } else { self.period };
        self.enabled = self.period != 0 || self.shift != 0;
        self.calc_made = false;

        if self.shift > 0 {
            let (freq, ret) = self.calculate();
            self.freq = freq;
            ret
        } else {
            false
        }
    }

    pub fn power_on(&mut self) {
        self.counter %= 8192;
    }

    pub fn read_nr10(&self) -> u8 {
        (0x80 | self.shift | (self.period << 4))
            .set_bit(3, self.negate)
            .u8()
    }

    // ret: if sound channel should be disabled.
    #[must_use]
    pub fn write_nr10(&mut self, value: u8) -> bool {
        self.period = (value >> 4) & 7;
        let old_negate = mem::replace(&mut self.negate, value.is_bit(3));
        self.shift = value & 7;

        if self.negate && !old_negate {
            self.calc_made = false;
        }
        if self.calc_made && old_negate && !self.negate {
            self.enabled = false;
            true
        } else {
            false
        }
    }
}
