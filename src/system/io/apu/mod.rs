//! This APU implementation is based on gamelin's, just like the rest of the emulator.
//! Gamelin's implementation was abridged from stan-roelofs's emulator: https://github.com/stan-roelofs/Kotlin-Gameboy-Emulator
//! Who probably abridged it from trekawek's coffee-gb: https://github.com/trekawek/coffee-gb
//! Thank you to both stan-roelofs and trekawek!

use crate::numutil::NumExt;
use crate::system::io::addr::*;
use crate::system::io::apu::channel::Channel;
use crate::system::io::apu::misc::LengthCounter;
use crate::system::io::Mmu;
use crate::system::T_CLOCK_HZ;
use std::mem;

mod channel;
mod misc;
mod noise;
mod square;

pub const SAMPLE_RATE: u32 = 22050;
const DIVIDER: usize = T_CLOCK_HZ / SAMPLE_RATE as usize;

pub struct Apu {
    channels: [Channel; 4],
    enabled: bool,

    left_vol: u8,
    right_vol: u8,
    left_enables: [bool; 4],
    right_enables: [bool; 4],

    out_div: usize,
    pub samples: Vec<i16>,
}

impl Apu {
    pub fn step(&mut self, t_cycles: usize) {
        self.out_div += t_cycles;
        if self.out_div < DIVIDER {
            for ch in &mut self.channels {
                ch.cycle(t_cycles as u16);
            }
        } else {
            self.out_div -= DIVIDER;
            let mut left = 0;
            let mut right = 0;

            for (i, ch) in self.channels.iter_mut().enumerate() {
                let sample = ch.cycle(t_cycles as u16).u16();
                left += sample * self.left_enables[i] as u16;
                right += sample * self.right_enables[i] as u16;
            }

            left *= self.left_vol.u16() + 1;
            right *= self.right_vol.u16() + 1;
            left *= 4;
            right *= 4;

            self.samples.push(left as i16);
            self.samples.push(right as i16);
        }
    }

    pub fn read(mmu: &Mmu, addr: u16) -> u8 {
        let chs = &mmu.apu.channels;
        match addr {
            NR10 => chs[0].kind.sweep().read_nr10(),
            NR11 => (chs[0].kind.sq().duty << 6) | 0x3F,
            NR12 => chs[0].vol_envelope.read_nr2(),
            NR14 => 0b1011_1111u8.set_bit(6, chs[0].len_counter.enabled).u8(),

            NR21 => (chs[1].kind.sq().duty << 6) | 0x3F,
            NR22 => chs[1].vol_envelope.read_nr2(),
            NR24 => 0b1011_1111u8.set_bit(6, chs[1].len_counter.enabled).u8(),

            NR34 => 0b1011_1111u8.set_bit(6, chs[2].len_counter.enabled).u8(),

            NR42 => chs[3].vol_envelope.read_nr2(),
            NR44 => 0b1011_1111u8.set_bit(6, chs[3].len_counter.enabled).u8(),

            NR52 => {
                let mut res = 0x70;
                for (i, ch) in chs.iter().enumerate() {
                    res = res.set_bit(i as u16, ch.enabled).u8();
                }
                res.set_bit(7, mmu.apu.enabled).u8()
            }

            _ => mmu[addr],
        }
    }

    pub fn write(mmu: &mut Mmu, addr: u16, value: u8) {
        // When powered off, all registers (NR10-NR51) are instantly written with zero and any writes to those
        // registers are ignored while power remains off (except on the DMG, where length counters are
        // unaffected by power and can still be written while off)
        if !mmu.apu.enabled
            && !mmu.cgb
            && addr != NR52
            && addr != NR11
            && addr != NR21
            && addr != NR31
            && addr != NR41
        {
            return;
        }

        let chs = &mut mmu.apu.channels;
        match addr {
            NR10 => chs[0].enabled &= !chs[0].kind.sweep_mut().write_nr10(value),
            NR11 => {
                if !chs[0].kind.sq().off {
                    chs[0].kind.sq_mut().duty = value >> 6;
                }
                chs[0].len_counter.write_nr1(value & 0x3F);
            }
            NR12 => {
                chs[0].vol_envelope.write_nr2(value);
                chs[0].enabled &= chs[0].vol_envelope.is_dac();
            }
            NR13 => *chs[0].kind.freq_mut() = (chs[0].kind.freq() & 0x700) | value.u16(),
            NR14 => {
                *chs[0].kind.freq_mut() = (chs[0].kind.freq() & 0xFF) | ((value.u16() & 7) << 8);
                LengthCounter::write_nr4(&mut chs[0], value);
                if value.is_bit(7) {
                    chs[0].trigger();
                }
            }

            NR21 => {
                if !chs[1].kind.sq().off {
                    chs[1].kind.sq_mut().duty = value >> 6;
                }
                chs[1].len_counter.write_nr1(value & 0x3F);
            }
            NR22 => {
                chs[1].vol_envelope.write_nr2(value);
                chs[1].enabled &= chs[1].vol_envelope.is_dac();
            }
            NR23 => *chs[1].kind.freq_mut() = (chs[1].kind.freq() & 0x700) | value.u16(),
            NR24 => {
                *chs[1].kind.freq_mut() = (chs[1].kind.freq() & 0xFF) | ((value.u16() & 7) << 8);
                LengthCounter::write_nr4(&mut chs[1], value);
                if value.is_bit(7) {
                    chs[1].trigger();
                }
            }

            NR30 => {
                *chs[2].kind.dac_mut() = value.is_bit(7);
                chs[2].enabled &= value.is_bit(7);
                mmu[NR30] = value | 0x7F;
            }
            NR31 => chs[2].len_counter.write_nr1(value),
            NR32 => {
                let code = (value & 0x60) >> 5;
                *chs[2].kind.vol_code_mut() = code;
                *chs[2].kind.vol_shift_mut() = if code == 0 { 4 } else { code - 1 };
                mmu[NR32] = value | 0x9F;
            }
            NR33 => *chs[2].kind.freq_mut() = (chs[2].kind.freq() & 0x700) | value.u16(),
            NR34 => {
                *chs[2].kind.freq_mut() = (chs[2].kind.freq() & 0xFF) | ((value.u16() & 7) << 8);
                LengthCounter::write_nr4(&mut chs[2], value);
                if value.is_bit(7) {
                    chs[2].trigger();
                }
            }

            NR41 => chs[3].len_counter.write_nr1(value & 0x3F),
            NR42 => {
                chs[3].vol_envelope.write_nr2(value);
                chs[3].enabled &= chs[3].vol_envelope.is_dac();
            }
            NR43 => {
                chs[3].kind.pc_mut().write_nr43(value);
                mmu[NR43] = value;
            }
            NR44 => {
                LengthCounter::write_nr4(&mut chs[3], value);
                if value.is_bit(7) {
                    chs[3].trigger();
                }
            }

            NR50 => {
                mmu.apu.left_vol = (value >> 4) & 0b111;
                mmu.apu.right_vol = value & 0b111;
                mmu[NR50] = value;
            }
            NR51 => {
                for i in 0..4 {
                    mmu.apu.left_enables[i] = value.is_bit(i as u16 + 4);
                    mmu.apu.right_enables[i] = value.is_bit(i as u16);
                }
                mmu[NR51] = value;
            }
            NR52 => {
                let was_enabled = mem::replace(&mut mmu.apu.enabled, value.is_bit(7));
                if was_enabled && !mmu.apu.enabled {
                    for ch in &mut mmu.apu.channels {
                        ch.power_off();
                    }
                    mmu.apu.left_vol = 0;
                    mmu.apu.right_vol = 0;
                    mmu.apu.left_enables = [false; 4];
                    mmu.apu.right_enables = [false; 4];
                } else if !was_enabled && mmu.apu.enabled {
                    for ch in &mut mmu.apu.channels {
                        ch.power_on();
                    }
                }
            }

            WAV_START..=WAV_END => {
                chs[2].kind.ram_mut()[(addr - WAV_START).us()] = value;
                mmu[addr] = value;
            }

            _ => (),
        }
    }

    pub fn new() -> Self {
        Self {
            channels: [
                Channel::new_sq1(),
                Channel::new_sq2(),
                Channel::new_wave(),
                Channel::new_noise(),
            ],
            enabled: true,

            left_vol: 7,
            right_vol: 7,
            left_enables: [true; 4],
            right_enables: [true, true, false, false],

            out_div: 0,
            samples: vec![],
        }
    }
}
