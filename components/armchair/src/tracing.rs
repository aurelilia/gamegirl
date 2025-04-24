use alloc::{
    format,
    string::{String, ToString},
};
use core::fmt::Write;

use common::numutil::NumExt;

use crate::{
    arm::ArmInst,
    interface::{Bus, CpuVersion},
    registers::{Flag, Register},
    thumb::ThumbInst,
    Cpu,
};

impl<S: Bus> Cpu<S> {
    pub fn trace_inst<TY: NumExt + 'static>(&mut self, inst: u32) {
        if self.debugger.tracing() {
            let cpsr = self.regs.cpsr();
            let mnem = self.get_inst_mnemonic(inst);

            let mut buf = String::with_capacity(100);
            let num = ('4' as u8 + S::Version::IS_V5 as u8) as char;
            buf.push(num);
            for reg in Register::from_rlist(u16::MAX) {
                write!(buf, "{:08X} ", self.regs[reg]).ok();
            }

            if TY::WIDTH == 2 {
                self.debugger.add_traced_instruction(|| {
                    format!("{buf}cpsr: {cpsr:08X} |     {inst:04X}: {mnem}")
                });
            } else {
                self.debugger.add_traced_instruction(|| {
                    format!("{buf}cpsr: {cpsr:08X} | {inst:08X}: {mnem}")
                });
            }
        }
    }

    pub fn get_inst_mnemonic(&mut self, inst: u32) -> String {
        if self.regs.is_flag(Flag::Thumb) {
            ThumbInst::of(inst.u16()).to_string()
        } else {
            ArmInst::of(inst.u32()).to_string()
        }
    }
}
