// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

/// A macro that can be used by systems to implement common functions
/// in a generic manner, as part of the system trait.
/// This macro simply grew as it became clear that some functionality
/// is easily shared between systems.
#[macro_export]
macro_rules! common_functions {
    ($clock:expr, $pause_event:expr, $size:expr) => {
        fn advance_delta(&mut self, delta: f32) {
            if !self.c.debugger.running {
                return;
            }

            let target =
                ($clock as f32 * delta * self.c.options.speed_multiplier as f32) as ::common::TimeS;
            self.scheduler.schedule($pause_event, target);

            self.c.in_tick = true;
            while self.c.debugger.running && self.c.in_tick {
                self.advance();
            }

            if self.c.audio_buffer.input[0].len() > 100_000 {
                self.c.audio_buffer.input[0].truncate(100);
                self.c.audio_buffer.input[1].truncate(100);
            }
        }

        #[cfg(feature = "serde")]
        fn save_state(&mut self) -> Vec<u8> {
            ::common::serialize::serialize(self, self.c.config.compress_savestates)
        }

        #[cfg(feature = "serde")]
        fn load_state(&mut self, state: &[u8]) {
            let old_self = mem::replace(
                self,
                ::common::serialize::deserialize(state, self.c.config.compress_savestates),
            );
            self.restore_from(old_self);
        }

        #[cfg(not(feature = "serde"))]
        fn save_state(&mut self) -> Vec<u8> {
            vec![]
        }

        #[cfg(not(feature = "serde"))]
        fn load_state(&mut self, state: &[u8]) {}

        fn get_time(&self) -> ::common::Time {
            self.scheduler.now()
        }

        fn screen_size(&self) -> [usize; 2] {
            $size
        }

        fn c(&self) -> &::common::Common {
            &self.c
        }

        fn c_mut(&mut self) -> &mut ::common::Common {
            &mut self.c
        }

        fn as_any(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}
