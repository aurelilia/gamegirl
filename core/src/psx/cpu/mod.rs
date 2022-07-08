// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod cop0;
mod inst;

use serde::{Deserialize, Serialize};

use crate::{
    numutil::NumExt,
    psx::{cpu::cop0::Cop0, PlayStation},
};

#[derive(Deserialize, Serialize)]
pub struct Cpu {
    regs: [u32; 32],
    pc: u32,
    pipeline: u32,

    cop0: Cop0,
    hi: u32,
    lo: u32,
}

impl Cpu {
    pub fn execute_next(ps: &mut PlayStation) {
        let inst = ps.cpu.pipeline;
        ps.cpu.pipeline = ps.read_word(ps.cpu.pc);
        ps.cpu.pc += 4;
        ps.run_inst(inst);
    }

    fn reg(&self, idx: u32) -> u32 {
        self.regs[idx.us()]
    }

    fn set_reg(&mut self, idx: u32, value: u32) {
        if idx == 0 {
            return;
        }
        self.regs[idx.us()] = value;
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            regs: [0; 32],
            pc: 0xBFC0_0000,
            pipeline: 0,

            cop0: Cop0::default(),
            hi: 0,
            lo: 0,
        }
    }
}

impl PlayStation {
    fn set_pc(&mut self, value: u32) {
        self.cpu.pc = value;
    }
}
