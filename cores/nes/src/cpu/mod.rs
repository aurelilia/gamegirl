// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod inst;
use common::numutil::{NumExt, U16Ext};
use modular_bitfield::bitfield;

use self::inst::Inst;
use crate::Nes;

#[bitfield]
#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct CpuStatus {
    carry: bool,
    zero: bool,
    interrupt_disable: bool,
    demimal_mode: bool,
    break_cmd: bool,
    overflow: bool,
    negative: bool,
    _unused: bool,
}

impl CpuStatus {
    fn set_zn(&mut self, value: u8) {
        self.set_zero(value == 0);
        self.set_negative(value.is_bit(7));
    }

    fn set_znc(&mut self, value: u16) {
        self.set_zero(value.u8() == 0);
        self.set_negative(value.is_bit(7));
        self.set_carry(value & 0xFF != 0);
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cpu {
    pub pc: u16,
    regs: [u8; 4],
    status: CpuStatus,
}

impl Cpu {
    pub fn exec_next_inst(nes: &mut Nes) {
        let inst = nes.read_imm();
        inst::execute(nes, Inst(inst));
    }

    pub fn trigger_int(nes: &mut Nes) {
        nes.push(nes.cpu.pc.low());
        nes.push(nes.cpu.pc.high());
        nes.push(nes.cpu.status.into());
        nes.cpu.status.set_break_cmd(true);
        nes.cpu.pc = 0xFFFE;
    }

    pub fn get(&self, reg: Reg) -> u8 {
        self.regs[reg as usize]
    }

    pub fn set(&mut self, reg: Reg, value: u8) {
        self.regs[reg as usize] = value;
    }
}

#[derive(Copy, Clone)]
pub enum Reg {
    A,
    X,
    Y,
    S,
}
