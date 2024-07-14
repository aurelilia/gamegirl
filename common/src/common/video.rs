// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::collections::VecDeque;

use crate::Colour;

/// Frame buffer for video output. Also used to implement frameskip.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FrameBuffer {
    /// Buffer of frames to be displayed.
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    buffer: VecDeque<Vec<Colour>>,
    /// Number of frames to skip before adding a frame to the buffer.
    pub frameskip: usize,
    /// Number of frames until the next frame is added to the buffer.
    n_until_next: usize,
}

impl FrameBuffer {
    /// Get the oldest frame in the buffer.
    pub fn pop(&mut self) -> Option<Vec<Colour>> {
        self.buffer.pop_front()
    }

    /// Get the newest framen in the buffer.
    pub fn pop_recent(&mut self) -> Option<Vec<Colour>> {
        self.buffer.pop_back()
    }

    /// Notify the buffer that the system is starting to render the next frame.
    /// Returns true if the frame should be rendered, false if it is to be
    /// skipped.
    pub fn start_next_frame(&mut self) -> bool {
        self.n_until_next = self.n_until_next.saturating_sub(1);
        self.n_until_next == 0
    }

    /// Push a new frame to the buffer.
    pub fn push(&mut self, frame: Vec<Colour>) {
        self.buffer.push_back(frame);
        if self.buffer.len() > 4 {
            self.pop(); // Drop oldest frame to prevent large buffer and
                        // outdated frames
        }
    }

    /// Do we have a frame?
    pub fn has_frame(&self) -> bool {
        !self.buffer.is_empty()
    }
}
