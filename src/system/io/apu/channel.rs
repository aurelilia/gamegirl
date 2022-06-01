use crate::numutil::NumExt;
use crate::system::io::apu::channel::Kind::{Noise, Square1, Square2, Wave};
use crate::system::io::apu::misc::{LengthCounter, VolumeEnvelope};
use crate::system::io::apu::noise::{Lfsr, PolyCounter};
use crate::system::io::apu::square::{FreqSweep, SquareCh, DUTY_CYCLES};

pub struct Channel {
    pub enabled: bool,
    last_output: u8,

    pub len_counter: LengthCounter,
    pub vol_envelope: VolumeEnvelope,

    pub kind: Kind,
}

impl Channel {
    pub fn cycle(&mut self, cycles: u16) -> u8 {
        match &mut self.kind {
            Square1 { sweep, .. } => self.enabled &= !sweep.cycle(cycles),
            _ => (),
        }

        self.vol_envelope.cycle(cycles);
        LengthCounter::cycle(self, cycles);

        let freq = self.kind.freq_adj();
        match &mut self.kind {
            Kind::Square1 { sq, .. } | Kind::Square2 { sq, .. } => {
                sq.timer -= cycles as i16;
                while sq.timer < 0 {
                    if freq == 0 {
                        sq.timer = 0;
                    } else {
                        sq.timer += freq as i16 * 4;
                    }
                    self.last_output = DUTY_CYCLES[sq.duty.us()].bit(sq.duty_counter).u8();
                    sq.duty_counter = (sq.duty_counter + 1) & 7;
                }
            }

            Kind::Wave {
                vol_shift,
                pattern_ram,
                freq,
                timer,
                pos_counter,
                ..
            } if self.enabled => {
                for _ in 0..cycles {
                    *timer -= 1;
                    if *timer == 0 {
                        *timer = (2048 - *freq) * 2;
                        self.last_output = pattern_ram[(*pos_counter / 2).us()];
                        if *pos_counter % 2 == 0 {
                            self.last_output = self.last_output >> 4;
                        }
                        self.last_output = self.last_output & 0xF >> *vol_shift;
                        *pos_counter = (*pos_counter + 1) & 31;
                    }
                }
                return self.last_output;
            }
            Wave { .. } => return 0,

            Kind::Noise { pc, lfsr } => {
                if pc.cycle(cycles) {
                    self.last_output = lfsr.cycle(cycles, pc.width_7);
                }
            }
        }

        if !self.enabled {
            0
        } else {
            self.last_output * self.vol_envelope.vol
        }
    }

    pub fn power_on(&mut self) {
        self.vol_envelope.power_on();
        match &mut self.kind {
            Square1 { sq, sweep } => {
                sq.off = false;
                sweep.power_on();
            }
            Square2 { sq, .. } => sq.off = false,
            _ => (),
        }
    }

    pub fn power_off(&mut self) {
        match &mut self.kind {
            Square1 { .. } => *self = Self::new_sq1(),
            Square2 { .. } => *self = Self::new_sq2(),
            Noise { .. } => *self = Self::new_noise(),
            Wave { pattern_ram, .. } => {
                let ram = pattern_ram.clone();
                *self = Self::new_wave();
                *self.kind.ram_mut() = ram;
            }
        }
        self.enabled = false;
        match &mut self.kind {
            Square1 { sq, .. } | Kind::Square2 { sq, .. } => {
                sq.off = true;
                sq.duty = 0;
                self.vol_envelope.write_nr2(0);
            }
            Wave { .. } => {}

            _ => (),
        }
    }

    pub fn trigger(&mut self) {
        self.vol_envelope.trigger();
        self.enabled = self.vol_envelope.is_dac();

        let freq = self.kind.freq_adj();
        match &mut self.kind {
            Square1 { sq, sweep } => {
                sq.timer = freq as i16 * 4;
                self.enabled &= !sweep.trigger();
            }

            Square2 { sq, .. } => sq.timer = freq as i16 * 4,

            Wave {
                timer,
                pos_counter,
                dac,
                ..
            } => {
                self.enabled = *dac;
                *timer = (2048 - freq) * 2;
                *pos_counter = 0;
            }

            Noise { pc, lfsr } => {
                pc.trigger();
                lfsr.0 = 0x7FFF;
            }
        }
    }

    pub fn new_sq1() -> Self {
        let mut ch = Self {
            enabled: true,
            last_output: 0,
            len_counter: LengthCounter::new(64),
            vol_envelope: VolumeEnvelope::default(),
            kind: Square1 {
                sq: SquareCh {
                    duty: 2,
                    duty_counter: 0,
                    off: false,
                    timer: 0,
                },
                sweep: FreqSweep::default(),
            },
        };
        ch.vol_envelope.write_nr2(0xF3);
        ch
    }

    pub fn new_sq2() -> Self {
        Self {
            enabled: false,
            last_output: 0,
            len_counter: LengthCounter::new(64),
            vol_envelope: VolumeEnvelope::default(),
            kind: Square2 {
                sq: SquareCh {
                    duty: 0,
                    duty_counter: 0,
                    off: true, // TODO false??
                    timer: 0,
                },
                freq: 0,
            },
        }
    }

    pub fn new_wave() -> Self {
        Self {
            enabled: false,
            last_output: 0,
            len_counter: LengthCounter::new(256),
            vol_envelope: VolumeEnvelope::default(),
            kind: Wave {
                vol_shift: 4,
                vol_code: 0,
                dac: false, // TODO not reset?
                pattern_ram: [0; 0x10],
                freq: 0,
                timer: 0,
                pos_counter: 0,
            },
        }
    }

    pub fn new_noise() -> Self {
        let mut ch = Self {
            enabled: false,
            last_output: 0,
            len_counter: LengthCounter::new(64),
            vol_envelope: VolumeEnvelope::default(),
            kind: Noise {
                pc: PolyCounter::default(),
                lfsr: Lfsr(0x7FFF),
            },
        };
        ch.len_counter.write_nr1(0xFF);
        ch
    }
}

pub enum Kind {
    Square1 {
        sq: SquareCh,
        sweep: FreqSweep,
    },

    Square2 {
        sq: SquareCh,
        freq: u16,
    },

    Wave {
        vol_shift: u8,
        vol_code: u8,
        dac: bool,
        pattern_ram: [u8; 0x10],
        freq: u16,
        timer: u16,
        pos_counter: u8,
    },

    Noise {
        pc: PolyCounter,
        lfsr: Lfsr,
    },
}

impl Kind {
    pub fn sq(&self) -> &SquareCh {
        match self {
            Kind::Square1 { sq, .. } | Kind::Square2 { sq, .. } => sq,
            _ => panic!("Not a square channel"),
        }
    }

    pub fn sq_mut(&mut self) -> &mut SquareCh {
        match self {
            Kind::Square1 { sq, .. } | Kind::Square2 { sq, .. } => sq,
            _ => panic!("Not a square channel"),
        }
    }

    pub fn sweep(&self) -> &FreqSweep {
        match self {
            Kind::Square1 { sweep, .. } => sweep,
            _ => panic!("Not Square1"),
        }
    }

    pub fn sweep_mut(&mut self) -> &mut FreqSweep {
        match self {
            Kind::Square1 { sweep, .. } => sweep,
            _ => panic!("Not Square1"),
        }
    }

    pub fn freq(&self) -> u16 {
        match self {
            Kind::Square1 { sweep, .. } => sweep.freq,
            Kind::Square2 { freq, .. } => *freq,
            Kind::Wave { freq, .. } => *freq,
            _ => 0,
        }
    }

    pub fn freq_adj(&self) -> u16 {
        match self {
            Kind::Square1 { sweep, .. } => 2048 - sweep.freq,
            Kind::Square2 { freq, .. } => 2048 - *freq,
            Kind::Wave { freq, .. } => *freq,
            _ => 0,
        }
    }

    pub fn freq_mut(&mut self) -> &mut u16 {
        match self {
            Kind::Square1 { sweep, .. } => &mut sweep.freq,
            Kind::Square2 { freq, .. } => freq,
            Kind::Wave { freq, .. } => freq,
            _ => panic!("Not a frequency channel"),
        }
    }

    pub fn ram_mut(&mut self) -> &mut [u8; 0x10] {
        match self {
            Kind::Wave { pattern_ram, .. } => pattern_ram,
            _ => panic!("Not wave channel"),
        }
    }

    pub fn dac_mut(&mut self) -> &mut bool {
        match self {
            Kind::Wave { dac, .. } => dac,
            _ => panic!("Not wave channel"),
        }
    }

    pub fn vol_code_mut(&mut self) -> &mut u8 {
        match self {
            Kind::Wave { vol_code, .. } => vol_code,
            _ => panic!("Not wave channel"),
        }
    }

    pub fn vol_shift_mut(&mut self) -> &mut u8 {
        match self {
            Kind::Wave { vol_shift, .. } => vol_shift,
            _ => panic!("Not wave channel"),
        }
    }

    pub fn pc_mut(&mut self) -> &mut PolyCounter {
        match self {
            Kind::Noise { pc, .. } => pc,
            _ => panic!("Not noise channel"),
        }
    }
}
