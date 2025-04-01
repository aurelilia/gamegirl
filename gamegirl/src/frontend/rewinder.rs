// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{iter, vec::Vec};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct RewinderConfig {
    /// Fast forward speed for the hold button.
    pub fast_forward_hold_speed: usize,
    /// Fast forward speed for the toggle button.
    pub fast_forward_toggle_speed: usize,
    /// Enable rewinding.
    pub enable_rewind: bool,
    /// Rewind buffer size (if enabled), in seconds.
    pub rewind_buffer_size: usize,
}

impl Default for RewinderConfig {
    fn default() -> Self {
        Self {
            fast_forward_hold_speed: 2,
            fast_forward_toggle_speed: 2,
            enable_rewind: true,
            rewind_buffer_size: 10,
        }
    }
}

/// Struct for storing rewind state.
/// "Rewinding" in the context of this is considered anything that 'turns back'
/// the clock - both literal frame-by-frame rewinding, and savestates.
pub struct Rewinder<const SAVESTATES: usize> {
    /// Save states that the user can store/load at any time.
    pub save_states: [Option<Vec<u8>>; SAVESTATES],
    /// Save state created before the last load, to allow the user
    /// to undo a load.
    pub before_last_ss_load: Option<Vec<u8>>,
    /// Rewind buffer.
    pub rewind_buffer: RWBuffer,
    /// If the emulation is currently rewinding.
    /// If we are, then instead of advancing the system normally, we load the
    /// saved state from the frame before out of the rewind buffer,
    /// then advance the system to make the PPU produce the previous frame to
    /// the user. Doing this every frame effectively makes the emulation run
    /// backward while still producing display output.
    /// Audio is also reversed by reversing the samples.
    pub rewinding: bool,
}

impl<const SAVESTATES: usize> Rewinder<SAVESTATES> {
    /// Set the size of the rewind buffer in seconds.
    pub fn set_rw_buf_size(&mut self, secs: usize) {
        self.rewind_buffer = RWBuffer::new(secs);
    }

    pub fn new(buffer_secs: usize) -> Self {
        Self {
            save_states: [const { None }; SAVESTATES],
            before_last_ss_load: None,
            rewind_buffer: RWBuffer::new(buffer_secs),
            rewinding: false,
        }
    }
}

/// Rewind buffer. Implemented as a simple LILO buffer on top of a `Vec`.
pub struct RWBuffer {
    vec: Vec<Vec<u8>>,
    cur_idx: usize,
    stop_idx: usize,
}

impl RWBuffer {
    /// Pop a state off the buffer. Will return None if the buffer is empty.
    pub fn pop(&mut self) -> Option<&[u8]> {
        if self.cur_idx == self.stop_idx {
            return None;
        }
        if self.cur_idx == 0 {
            self.cur_idx = self.vec.len() - 1;
        } else {
            self.cur_idx -= 1;
        };
        Some(&self.vec[self.cur_idx])
    }

    /// Push a new state to the buffer.
    pub fn push(&mut self, val: Vec<u8>) {
        if self.cur_idx == self.vec.len() - 1 {
            self.cur_idx = 0;
        } else {
            self.cur_idx += 1;
        }
        self.stop_idx = self.cur_idx + 1;
        self.vec[self.cur_idx] = val;
    }

    /// Create a new buffer with the given seconds of rewind storage.
    fn new(secs: usize) -> Self {
        Self {
            vec: iter::repeat(Vec::new()).take(60 * secs).collect(),
            cur_idx: 0,
            stop_idx: 0,
        }
    }
}
