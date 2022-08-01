#[macro_export]
macro_rules! common_functions {
    ($clock:expr, $pause_event:expr) => {
        /// Advance the system clock by the given delta in seconds.
        /// Might advance a few clocks more.
        pub fn advance_delta(&mut self, delta: f32) {
            if !self.options.running {
                return;
            }

            let target = ($clock as f32 * delta * self.options.speed_multiplier as f32) as i32;
            self.scheduler.schedule($pause_event, target);

            self.ticking = true;
            while self.options.running && self.ticking {
                self.advance();
            }
        }

        /// Step until the PPU has finished producing the current frame.
        /// Only used for rewinding since it causes audio desync very easily.
        pub fn produce_frame(&mut self) -> Option<Vec<Colour>> {
            while self.options.running && self.ppu.last_frame == None {
                self.advance();
            }
            self.ppu.last_frame.take()
        }

        /// Produce the next audio samples and write them to the given buffer.
        /// Writes zeroes if the system is not currently running
        /// and no audio should be played.
        pub fn produce_samples(&mut self, samples: &mut [f32]) {
            if !self.options.running {
                samples.fill(0.0);
                return;
            }

            let target = samples.len() * self.options.speed_multiplier;
            while self.apu.buffer.len() < target {
                if !self.options.running {
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
                self.apu.buffer.truncate(5_000);

                for (src, dst) in buffer
                    .into_iter()
                    .step_by(self.options.speed_multiplier)
                    .zip(samples.iter_mut())
                {
                    *dst = src * self.config.volume;
                }
            }
        }

        /// Reset the console, while keeping the current cartridge inserted.
        pub fn reset(&mut self) {
            let old_self = mem::take(self);
            self.restore_from(old_self);
        }

        /// Create a save state that can be loaded with [load_state].
        pub fn save_state(&self) -> Vec<u8> {
            common::misc::serialize(self, self.config.compress_savestates)
        }

        /// Load a state produced by [save_state].
        /// Will restore the current cartridge and debugger.
        pub fn load_state(&mut self, state: &[u8]) {
            if cfg!(target_arch = "wasm32") {
                // Currently crashes...
                return;
            }

            let old_self = mem::replace(
                self,
                common::misc::deserialize(state, self.config.compress_savestates),
            );
            self.restore_from(old_self);
        }
    };
}
