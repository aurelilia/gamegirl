use crate::numutil::NumExt;
use crate::system::io::apu::channel::Channel;
use crate::system::T_CLOCK_HZ;
use std::mem;

const LEN_DIVIDER: u16 = (T_CLOCK_HZ / 256) as u16;
const VOL_DIVIDER: u32 = (T_CLOCK_HZ / 64) as u32;

pub struct LengthCounter {
    full_length: u16,
    length: u16,
    pub(crate) counter: u16,
    pub(crate) enabled: bool,
}

impl LengthCounter {
    pub fn cycle(ch: &mut Channel, cycles: u16) {
        ch.len_counter.counter += cycles;
        if ch.len_counter.counter >= LEN_DIVIDER {
            ch.len_counter.counter -= LEN_DIVIDER;
            if ch.len_counter.enabled && ch.len_counter.length > 0 {
                Self::decrease_length(ch);
            }
        }
    }

    pub fn write_nr1(&mut self, value: u8) {
        self.length = self.full_length.wrapping_sub(value.u16());
    }

    pub fn write_nr4(ch: &mut Channel, value: u8) {
        let was_enabled = mem::replace(&mut ch.len_counter.enabled, value.is_bit(6));

        if !was_enabled
            && ch.len_counter.enabled
            && ch.len_counter.length != 0
            && ch.len_counter.counter < (LEN_DIVIDER / 2)
        {
            Self::decrease_length(ch);
        }
        if value.is_bit(7) && ch.len_counter.length == 0 {
            ch.len_counter.length = ch.len_counter.full_length;
            if ch.len_counter.counter < (LEN_DIVIDER / 2) && ch.len_counter.enabled {
                Self::decrease_length(ch);
            }
        }
    }

    fn decrease_length(ch: &mut Channel) {
        ch.len_counter.length -= 1;
        if ch.len_counter.length == 0 {
            ch.enabled = false;
        }
    }

    pub fn new(full_length: u16) -> Self {
        Self {
            full_length,
            length: full_length,
            counter: 0,
            enabled: false,
        }
    }
}

#[derive(Default)]
pub struct VolumeEnvelope {
    add: bool,
    starting_vol: u8,
    pub vol: u8,
    period: u8,
    counter: u32,
    enabled: bool,
}

impl VolumeEnvelope {
    pub fn cycle(&mut self, cycles: u16) {
        self.counter += cycles as u32;
        if self.period > 0 && self.counter >= (self.period as u32 * VOL_DIVIDER) {
            self.counter = 0;
            if self.enabled && self.period > 0 {
                let new_vol = if self.add {
                    self.vol as i8 + 1
                } else {
                    self.vol as i8 - 1
                };
                if new_vol < 0 || new_vol > 15 {
                    self.enabled = false
                } else {
                    self.vol = new_vol as u8;
                }
            }
        }
    }

    pub fn power_on(&mut self) {
        self.counter %= 8192;
    }

    pub fn trigger(&mut self) {
        self.vol = self.starting_vol;
        self.enabled = true;
    }

    pub fn is_dac(&self) -> bool {
        self.starting_vol != 0 || self.add
    }

    pub fn read_nr2(&self) -> u8 {
        (self.starting_vol << 4 | self.period)
            .set_bit(3, self.add)
            .u8()
    }

    pub fn write_nr2(&mut self, value: u8) {
        self.starting_vol = value >> 4;
        self.add = value.is_bit(3);
        self.period = value & 7;
    }
}
