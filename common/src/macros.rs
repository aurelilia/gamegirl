// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

/// A macro that can be used by systems to implement common functions
/// in a generic manner, as part of the system trait.
/// This macro simply grew as it became clear that some functionality
/// is easily shared between systems.
#[macro_export]
macro_rules! common_functions {
    ($clock:expr, $pause_event:expr, $size:expr) => {
        fn advance_delta(&mut self, delta: f32) {
            if !self.debugger.running {
                return;
            }

            let target = ($clock as f32 * delta * self.options.speed_multiplier as f32) as i32;
            self.scheduler.schedule($pause_event, target);

            self.ticking = true;
            while self.debugger.running && self.ticking {
                self.advance();
            }
        }

        fn produce_frame(&mut self) -> Option<Vec<::common::Colour>> {
            while self.debugger.running && self.ppu.last_frame == None {
                self.advance();
            }
            self.ppu.last_frame = None;

            // Do it twice: Color buffer will be empty after a save state load,
            // we need to render one frame in full
            while self.debugger.running && self.ppu.last_frame == None {
                self.advance();
            }
            self.ppu.last_frame.take()
        }

        fn is_running(&mut self) -> &mut bool {
            &mut self.debugger.running
        }

        #[cfg(feature = "serde")]
        fn save_state(&mut self) -> Vec<u8> {
            common::misc::serialize(self, self.config.compress_savestates)
        }

        #[cfg(feature = "serde")]
        fn load_state(&mut self, state: &[u8]) {
            let old_self = mem::replace(
                self,
                common::misc::deserialize(state, self.config.compress_savestates),
            );
            self.restore_from(old_self);
        }

        fn last_frame(&mut self) -> Option<Vec<::common::Colour>> {
            self.ppu.last_frame.take()
        }

        fn options(&mut self) -> &mut EmulateOptions {
            &mut self.options
        }

        fn config(&self) -> &SystemConfig {
            &self.config
        }

        fn config_mut(&mut self) -> &mut SystemConfig {
            &mut self.config
        }

        fn screen_size(&self) -> [usize; 2] {
            $size
        }

        fn as_any(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}

/// An implementation of [Core::produce_samples] for systems that regularly
/// push finished samples to an output buffer.
#[macro_export]
macro_rules! produce_samples_buffered {
    ($rate:expr) => {
        fn produce_samples(&mut self, samples: &mut [f32]) {
            if !self.debugger.running {
                samples.fill(0.0);
                return;
            }

            let target = samples.len() * self.options.speed_multiplier;
            while self.apu.buffer.len() < target {
                if !self.debugger.running {
                    samples.fill(0.0);
                    return;
                }
                self.advance();
            }

            let mut buffer = mem::take(&mut self.apu.buffer);
            if self.options.invert_audio_samples {
                // If rewinding, truncate and get rid of any excess samples to prevent
                // audio samples getting backed up
                for (src, dst) in buffer.into_iter().zip(samples.iter_mut().rev()) {
                    *dst = src * self.config.volume;
                }
            } else {
                // Otherwise, store any excess samples back in the buffer for next time
                // while again not storing too many to avoid backing up.
                // This way can cause clipping if the console produces audio too fast,
                // however this is preferred to audio falling behind and eating
                // a lot of memory.
                for sample in buffer.drain(target..) {
                    self.apu.buffer.push(sample);
                }
                if self.apu.buffer.len() > $rate as usize / 2 {
                    log::warn!("Audio samples are backing up! Truncating");
                    self.apu.buffer.truncate(100);
                }

                for (src, dst) in buffer
                    .into_iter()
                    .step_by(self.options.speed_multiplier)
                    .zip(samples.iter_mut())
                {
                    *dst = src * self.config.volume;
                }
            }
        }

        fn wanted_sample_rate(&self) -> u32 {
            $rate
        }
    };
}
