use crate::numutil::NumExt;

#[derive(Default)]
pub struct PolyCounter {
    shifted_divisor: u32,
    counter: u32,
    pub width_7: bool,
}

impl PolyCounter {
    pub fn cycle(&mut self, cycles: u16) -> bool {
        // TODO loop is definitely inefficient
        for _ in 0..cycles {
            self.counter = self.counter.wrapping_sub(1);
            if self.counter == 0 {
                self.trigger();
            }
        }
        self.counter == self.shifted_divisor * 4
    }

    pub fn trigger(&mut self) {
        self.counter = self.shifted_divisor * 4;
    }

    pub fn write_nr43(&mut self, value: u8) {
        let clock_shift = value >> 4;
        let div_code = value & 7;
        let divisor = if div_code == 0 { 8 } else { div_code << 4 };
        self.shifted_divisor = (divisor as u32) << clock_shift as u32;
        self.width_7 = value.is_bit(3);
    }
}

pub struct Lfsr(pub u16);

impl Lfsr {
    pub fn cycle(&mut self, cycles: u16, width_7: bool) -> u8 {
        for _ in 0..cycles {
            let x = self.0 & 1 ^ (self.0 & 2 >> 1) != 0;
            self.0 >>= 1;
            self.0 = self.0 | if x { 1 << 14 } else { 0 };
            if width_7 {
                self.0 = self.0 | if x { 1 << 6 } else { 0 };
            }
        }
        1 & (self.0.u8() ^ 1)
    }
}
