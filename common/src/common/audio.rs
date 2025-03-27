// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#[cfg(feature = "std")]
pub use std::*;

#[cfg(not(feature = "std"))]
pub use nostd::*;

#[cfg(not(feature = "std"))]
mod nostd {
    use alloc::vec::Vec;

    use crate::common::options::SystemConfig;

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    #[cfg_attr(feature = "serde_config", derive(serde::Deserialize, serde::Serialize))]
    pub enum AudioSampler {
        Nearest,
        Cubic,
    }

    #[derive(Default)]
    pub struct AudioBuffer {
        pub input: [Vec<f32>; 2],
        input_sr: usize,
        output_sr: usize,
        output_chunk_size: usize,
    }

    impl AudioBuffer {
        pub fn fill_buffer(&mut self, _buf: &mut [f32], _skip: usize, _volume: f32) {
            todo!()
        }

        pub fn can_fill_buffer(&self, _skip: usize) -> bool {
            todo!()
        }

        pub fn set_input_sr(&mut self, sr: usize) {
            self.input_sr = sr;
            self.reinit_sampler();
        }

        pub fn update_output_chunk_size(&mut self, size: usize) {
            if size == self.output_chunk_size {
                return;
            }
            self.output_chunk_size = size;
            self.reinit_sampler();
        }

        pub(crate) fn reinit_sampler(&mut self) {
            self.input.iter_mut().for_each(|v| v.clear());
        }

        pub fn with_config(config: &SystemConfig) -> Self {
            Self {
                input: [Vec::new(), Vec::new()],
                input_sr: 48000,
                output_sr: config.sample_rate,
                output_chunk_size: 512,
            }
        }
    }
}

#[cfg(feature = "std")]
mod std {
    use alloc::{boxed::Box, vec::Vec};
    use core::{
        fmt,
        fmt::{Display, Formatter},
    };
    use std::sync::Mutex;

    use rubato::{
        FastFixedOut, PolynomialDegree, SincFixedOut, SincInterpolationParameters,
        SincInterpolationType, VecResampler, WindowFunction,
    };

    use crate::common::options::SystemConfig;

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    #[cfg_attr(feature = "serde_config", derive(serde::Deserialize, serde::Serialize))]
    pub enum AudioSampler {
        Nearest,
        Linear,
        Cubic,
        SincLinear { len: usize },
        SincCubic { len: usize },
    }

    impl AudioSampler {
        pub const SAMPLERS: &[AudioSampler] = &[
            AudioSampler::Nearest,
            AudioSampler::Linear,
            AudioSampler::Cubic,
            AudioSampler::SincLinear { len: 64 },
            AudioSampler::SincLinear { len: 128 },
            AudioSampler::SincLinear { len: 256 },
            AudioSampler::SincCubic { len: 64 },
            AudioSampler::SincCubic { len: 128 },
            AudioSampler::SincCubic { len: 256 },
        ];
    }

    pub fn make_sampler(
        typ: AudioSampler,
        ratio: f64,
        output_buffer_len: usize,
    ) -> Box<dyn VecResampler<f32>> {
        match typ {
            AudioSampler::Nearest => Box::new(
                FastFixedOut::new(ratio, 1.0, PolynomialDegree::Nearest, output_buffer_len, 2)
                    .unwrap(),
            ),
            AudioSampler::Linear => Box::new(
                FastFixedOut::new(ratio, 1.0, PolynomialDegree::Linear, output_buffer_len, 2)
                    .unwrap(),
            ),
            AudioSampler::Cubic => Box::new(
                FastFixedOut::new(ratio, 1.0, PolynomialDegree::Cubic, output_buffer_len, 2)
                    .unwrap(),
            ),

            AudioSampler::SincLinear { len } => Box::new(
                SincFixedOut::new(
                    ratio,
                    1.0,
                    SincInterpolationParameters {
                        sinc_len: len,
                        f_cutoff: 0.95,
                        oversampling_factor: len,
                        interpolation: SincInterpolationType::Linear,
                        window: WindowFunction::BlackmanHarris2,
                    },
                    output_buffer_len,
                    2,
                )
                .unwrap(),
            ),
            AudioSampler::SincCubic { len } => Box::new(
                SincFixedOut::new(
                    ratio,
                    1.0,
                    SincInterpolationParameters {
                        sinc_len: len,
                        f_cutoff: 0.95,
                        oversampling_factor: len,
                        interpolation: SincInterpolationType::Cubic,
                        window: WindowFunction::BlackmanHarris2,
                    },
                    output_buffer_len,
                    2,
                )
                .unwrap(),
            ),
        }
    }

    impl Display for AudioSampler {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                AudioSampler::Nearest => write!(f, "Nearest"),
                AudioSampler::Linear => write!(f, "Linear"),
                AudioSampler::Cubic => write!(f, "Cubic"),
                AudioSampler::SincLinear { len } => write!(f, "Sinc-Linear-{len}"),
                AudioSampler::SincCubic { len } => write!(f, "Sinc-Cubic-{len}"),
            }
        }
    }

    pub struct AudioBuffer {
        sampler: Mutex<Box<dyn VecResampler<f32>>>,
        temp_output: [Vec<f32>; 2],
        next_frames: usize,

        pub input: [Vec<f32>; 2],
        input_sr: usize,
        output_chunk_size: usize,
        output_sr: usize,
        sampling: AudioSampler,
    }

    impl AudioBuffer {
        pub fn fill_buffer(&mut self, buf: &mut [f32], skip: usize, volume: f32) {
            assert_ne!(0, skip);
            assert_eq!(buf.len() / 2, self.temp_output[0].len());
            if skip != 1 {
                self.input.iter_mut().for_each(|v| {
                    *v = v.iter().step_by(skip).copied().collect();
                });
            }

            let mut sampler = self.sampler.lock().unwrap();
            let (used, written) = sampler
                .process_into_buffer(&self.input, &mut self.temp_output, None)
                .unwrap();
            assert_eq!(self.temp_output[0].len(), written);
            self.input.iter_mut().for_each(|v| {
                v.drain(..used);
            });

            for (i, v) in buf.iter_mut().enumerate() {
                *v = self.temp_output[i & 1][i >> 1] * volume;
            }

            if self.input[0].len() > self.input_sr / 2 {
                log::warn!("Audio samples are backing up! Truncating");
                self.input[0].truncate(500);
                self.input[1].truncate(500);
            }
            self.next_frames = sampler.input_frames_next();
        }

        pub fn can_fill_buffer(&self, skip: usize) -> bool {
            (self.next_frames * skip) <= self.input[0].len()
        }

        pub fn set_input_sr(&mut self, sr: usize) {
            self.input_sr = sr;
            self.reinit_sampler();
        }

        pub fn set_output_sr(&mut self, sr: usize) {
            self.output_sr = sr;
            self.reinit_sampler();
        }

        pub fn update_output_chunk_size(&mut self, size: usize) {
            if size == self.output_chunk_size {
                return;
            }
            self.output_chunk_size = size;
            self.reinit_sampler();
        }

        pub fn set_sampling(&mut self, sampling: AudioSampler) {
            self.sampling = sampling;
            self.reinit_sampler();
        }

        pub(crate) fn reinit_sampler(&mut self) {
            let size = self.output_chunk_size;
            self.temp_output[0].resize(size, 0.0);
            self.temp_output[1].resize(size, 0.0);
            self.sampler = Mutex::new(make_sampler(
                self.sampling,
                self.output_sr as f64 / self.input_sr as f64,
                size,
            ));
            self.input.iter_mut().for_each(|v| v.clear());
            self.next_frames = self.sampler.lock().unwrap().input_frames_next();
        }

        pub fn with_config(config: &SystemConfig) -> Self {
            let sampler = make_sampler(config.resampler, 1.0, 1024);
            let next_frames = sampler.input_frames_next();
            Self {
                sampler: Mutex::new(sampler),
                temp_output: [Vec::new(), Vec::new()],
                next_frames,
                input: [Vec::new(), Vec::new()],
                input_sr: 48000,
                output_sr: config.sample_rate,
                output_chunk_size: 1024,
                sampling: config.resampler,
            }
        }
    }

    impl Default for AudioBuffer {
        fn default() -> Self {
            Self::with_config(&SystemConfig::default())
        }
    }
}
