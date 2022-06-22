use crate::{
    gga::{
        addr::{FIFO_A_L, SOUNDBIAS, SOUNDCNT_H},
        dma::Dmas,
        GameGirlAdv, CPU_CLOCK,
    },
    numutil::{NumExt, U16Ext},
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const SAMPLE_RATE: f32 = 44100.;
const SAMPLE_EVERY_N_CLOCKS: f32 = CPU_CLOCK / SAMPLE_RATE;

#[derive(Default, Deserialize, Serialize)]
pub struct Apu {
    buffers: [VecDeque<i8>; 2],
    current_samples: [i8; 2],

    sample_counter: f32,
    pub buffer: Vec<f32>,
}

impl Apu {
    pub fn step(gg: &mut GameGirlAdv, cycles: u16) {
        gg.apu.sample_counter += cycles as f32;
        if gg.apu.sample_counter >= SAMPLE_EVERY_N_CLOCKS {
            Self::push_output(gg);
            gg.apu.sample_counter -= SAMPLE_EVERY_N_CLOCKS;
        }
    }

    fn push_output(gg: &mut GameGirlAdv) {
        let mut left = 0;
        let mut right = 0;

        let cnt = gg[SOUNDCNT_H];
        let a_vol_mul = 1 + cnt.bit(2) as i16;
        let b_vol_mul = 1 + cnt.bit(3) as i16;
        let a = gg.apu.current_samples[0] as i16 * a_vol_mul * 2;
        let b = gg.apu.current_samples[1] as i16 * b_vol_mul * 2;

        if cnt.is_bit(8) {
            right += a;
        }
        if cnt.is_bit(9) {
            left += a;
        }
        if cnt.is_bit(12) {
            right += b;
        }
        if cnt.is_bit(13) {
            left += b;
        }

        let bias = gg[SOUNDBIAS].bits(0, 10) as i16;
        gg.apu.buffer.push(Self::bias(right, bias) as f32 / 1024.0);
        gg.apu.buffer.push(Self::bias(left, bias) as f32 / 1024.0);
    }

    pub fn timer_overflow<const CH: usize>(gg: &mut GameGirlAdv) {
        if let Some(next) = gg.apu.buffers[CH].pop_front() {
            gg.apu.current_samples[CH] = next;
        }

        if gg.apu.buffers[CH].len() <= 16 {
            let dest = 0x400_0000 | (FIFO_A_L + CH.u32() * 4);
            for dma in 1..=2 {
                if Dmas::get_dest(gg, dma) == dest {
                    Dmas::check_special_transfer(gg, dma)
                }
            }
        }
    }

    pub fn push_samples<const CH: usize>(&mut self, samples: u16) {
        self.buffers[CH].push_back(samples.low() as i8);
        self.buffers[CH].push_back(samples.high() as i8);
    }

    pub fn push_sample<const CH: usize>(&mut self, samples: u8) {
        self.buffers[CH].push_back(samples as i8);
    }

    fn bias(mut sample: i16, bias: i16) -> i16 {
        sample += bias;
        if sample > 0x3ff {
            sample = 0x3ff;
        } else if sample < 0 {
            sample = 0;
        }
        sample -= bias;
        sample
    }
}
