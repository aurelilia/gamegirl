use std::iter;

/// Struct for storing rewind state.
pub struct Rewinding {
    /// Save states that the user can store/load at any time.
    pub save_states: [Option<Vec<u8>>; 10],
    /// Save state created before the last load, to allow the user
    /// to undo a load.
    pub before_last_ss_load: Option<Vec<u8>>,
    /// Rewind buffer.
    pub rewind_buffer: RWBuffer,
    /// If the emulation is currently rewinding.
    /// If we are, then instead of advancing the system normally, we load the
    /// saved state from the frame before out of the rewind buffer,
    /// then advance the system to make the PPU produce the previous frame to the user.
    /// Doing this every frame effectively makes the emulation run backward while
    /// still producing display output.
    /// Audio is also reversed by reversing the samples.
    pub rewinding: bool,
}

impl Rewinding {
    /// Set the size of the rewind buffer in seconds.
    pub fn set_rw_buf_size(&mut self, secs: usize) {
        self.rewind_buffer = RWBuffer::new(secs);
    }
}

impl Default for Rewinding {
    fn default() -> Self {
        Self {
            save_states: [None, None, None, None, None, None, None, None, None, None],
            before_last_ss_load: None,
            rewind_buffer: RWBuffer::new(60 * 10),
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
            vec: iter::repeat(vec![]).take(60 * secs).collect(),
            cur_idx: 0,
            stop_idx: 0,
        }
    }
}
