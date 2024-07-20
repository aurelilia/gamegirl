// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! CPU implementations.
//! Note that when it comes to timing, the ARM9 runs on the scheduler until
//! the ARM7 is behind, which then runs outside the scheduler until the ARM9 is
//! behind. This is repeated in a loop.
//! Effectively, the ARM9 is the one handling the scheduling, with the ARM7
//! being dragged along.

pub mod cp15;
pub mod math;
mod nds7;
mod nds9;

pub const NDS9_CLOCK: u32 = 67_027_964;
