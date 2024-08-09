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
use decode::*;
pub use decode::{make_thumb_lut, ThumbInst};

use super::interface::{ArmSystem, SysWrapper};

pub type ThumbHandler<I> = fn(&mut I, ThumbInst);
pub type ThumbLut<I> = [ThumbHandler<I>; 256];

impl<S: ArmSystem> SysWrapper<S> {
    pub fn execute_thumb(&mut self, inst: u16) {
        let handler = Self::get_handler_thumb(inst);
        handler(self, ThumbInst::of(inst));
    }

    pub fn get_handler_thumb(inst: u16) -> ThumbHandler<Self> {
        S::THUMB_LUT[inst.us() >> 8]
    }
}

trait ThumbExecutor {
    fn thumb_unknown_opcode(&mut self, inst: ThumbInst);
    fn thumb_arithmetic<const KIND: Thumb12Op>(&mut self, inst: ThumbInst);
    fn thumb_3<const KIND: Thumb3Op>(&mut self, inst: ThumbInst);
    fn thumb_alu(&mut self, inst: ThumbInst);
    fn thumb_hi_add(&mut self, inst: ThumbInst);
    fn thumb_hi_cmp(&mut self, inst: ThumbInst);
    fn thumb_hi_mov(&mut self, inst: ThumbInst);
    fn thumb_hi_bx(&mut self, inst: ThumbInst);
    fn thumb_ldr6(&mut self, inst: ThumbInst);
    fn thumb_ldrstr78<const O: ThumbStrLdrOp>(&mut self, inst: ThumbInst);
    fn thumb_ldrstr9<const O: ThumbStrLdrOp>(&mut self, inst: ThumbInst);
    fn thumb_ldrstr10<const STR: bool>(&mut self, inst: ThumbInst);
    fn thumb_str_sp(&mut self, inst: ThumbInst);
    fn thumb_ldr_sp(&mut self, inst: ThumbInst);
    fn thumb_rel_addr<const SP: bool>(&mut self, inst: ThumbInst);
    fn thumb_sp_offs(&mut self, inst: ThumbInst);
    fn thumb_push<const SP: bool>(&mut self, inst: ThumbInst);
    fn thumb_pop<const PC: bool>(&mut self, inst: ThumbInst);
    fn thumb_stmia(&mut self, inst: ThumbInst);
    fn thumb_ldmia(&mut self, inst: ThumbInst);
    fn thumb_bcond<const COND: u16>(&mut self, inst: ThumbInst);
    fn thumb_swi(&mut self, inst: ThumbInst);
    fn thumb_br(&mut self, inst: ThumbInst);
    fn thumb_set_lr(&mut self, inst: ThumbInst);
    fn thumb_bl<const THUMB: bool>(&mut self, inst: ThumbInst);
}
