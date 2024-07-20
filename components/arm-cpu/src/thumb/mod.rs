// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

mod decode;
mod execute;

use common::numutil::NumExt;
pub use decode::ThumbInst;

use super::interface::{ArmSystem, SysWrapper};

pub type ThumbHandler<S> = fn(&mut SysWrapper<S>, ThumbInst);
pub type ThumbLut<S> = [ThumbHandler<S>; 256];

impl<S: ArmSystem> SysWrapper<S> {
    pub fn execute_inst_thumb(&mut self, inst: u16) {
        let handler = Self::get_handler_thumb(inst);
        handler(self, ThumbInst::of(inst));
    }

    pub fn get_handler_thumb(inst: u16) -> ThumbHandler<S> {
        S::THUMB_LUT[inst.us() >> 8]
    }
}
