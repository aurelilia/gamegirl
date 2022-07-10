// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::{
    numutil::NumExt,
    psx::{
        cpu::inst::{Inst, InstructionHandler},
        PlayStation,
    },
};

type CopLut = [InstructionHandler; 32];
const COP0: CopLut = PlayStation::cop0_table();

#[derive(Default, Deserialize, Serialize)]
pub struct Cop0 {
    pub(crate) sr: u32,
    pub(crate) cause: u32,
    pub(crate) epc: u32,
}

impl PlayStation {
    const fn cop0_table() -> CopLut {
        let mut lut: CopLut = [Self::cop0_inst; 32];
        lut[0x00] = Self::mfc0;
        lut[0x02] = Self::cfc0;
        lut[0x04] = Self::mtc0;
        lut[0x06] = Self::ctc0;
        lut[0x08] = Self::bc0;
        lut[0x10] = Self::rfe;
        lut
    }
}

impl PlayStation {
    pub fn cop0(&mut self, inst: Inst) {
        let cop0 = inst.rs();
        let handler = COP0[cop0.us()];
        handler(self, inst);
    }

    fn mfc0(&mut self, inst: Inst) {
        match inst.rd() {
            12 => self.cpu.set_reg(inst.rt(), self.cpu.cop0.sr),
            13 => self.cpu.set_reg(inst.rt(), self.cpu.cop0.cause),
            14 => self.cpu.set_reg(inst.rt(), self.cpu.cop0.epc),
            unknown => log::debug!("Unhandled read from COP0 register {unknown}, ignoring"),
        }
    }

    fn cfc0(&mut self, inst: Inst) {
        todo!();
    }

    fn mtc0(&mut self, inst: Inst) {
        match inst.rd() {
            12 => self.cpu.cop0.sr = self.cpu.reg(inst.rt()),
            unknown => log::debug!("Unhandled write to COP0 register {unknown}, ignoring"),
        }
    }

    fn ctc0(&mut self, inst: Inst) {
        todo!();
    }

    fn bc0(&mut self, inst: Inst) {
        todo!();
    }

    fn rfe(&mut self, inst: Inst) {
        if inst.0 & 0x3F != 0x10 {
            log::warn!("COP0 virtual memory instruction encountered, executing as RFE");
        }

        let context = self.cpu.cop0.sr & 0x3F;
        self.cpu.cop0.sr &= !0x3F;
        self.cpu.cop0.sr |= context >> 2;
    }

    fn cop0_inst(&mut self, inst: Inst) {
        todo!();
    }
}
