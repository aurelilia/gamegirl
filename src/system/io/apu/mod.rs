use crate::system::io::Mmu;
use crate::system::T_CLOCK_HZ;

mod misc;
mod noise;
mod square;
mod wave;

pub const SAMPLE_RATE: u32 = 22050;
const DIVIDER: usize = T_CLOCK_HZ / SAMPLE_RATE as usize;

#[derive(Default)]
pub struct Apu {
    out_div: usize,

    pub samples: Vec<f32>,
}

impl Apu {
    pub fn step(mmu: &mut Mmu, t_cycles: usize) {
        mmu.apu.out_div += t_cycles;
        if mmu.apu.out_div < DIVIDER {
        } else {
            mmu.apu.out_div -= DIVIDER;
            mmu.apu.samples.push(0.0);
            mmu.apu.samples.push(0.0);
        }
    }
}
