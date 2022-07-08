use serde::{Deserialize, Serialize};

use crate::psx::{
    cpu::inst::{Inst, InstructionHandler},
    PlayStation,
};

type CopLut = [InstructionHandler; 32];
const COP0: CopLut = PlayStation::cop0_table();

#[derive(Default, Deserialize, Serialize)]
pub struct Cop0 {
    pub(crate) sr: u32,
    cause: u32,
}

impl PlayStation {
    const fn cop0_table() -> CopLut {
        let mut lut: CopLut = [Self::cop0_inst; 32];
        lut[0x00] = Self::mfc0;
        lut[0x02] = Self::cfc0;
        lut[0x04] = Self::mtc0;
        lut[0x06] = Self::ctc0;
        lut[0x08] = Self::bc0;
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
            _ => (),
        }
    }

    fn cfc0(&mut self, inst: Inst) {
        todo!();
    }

    fn mtc0(&mut self, inst: Inst) {
        match inst.rd() {
            12 => self.cpu.cop0.sr = self.cpu.reg(inst.rt()),
            _ => (),
        }
    }

    fn ctc0(&mut self, inst: Inst) {
        todo!();
    }

    fn bc0(&mut self, inst: Inst) {
        todo!();
    }

    fn cop0_inst(&mut self, inst: Inst) {
        todo!();
    }
}
