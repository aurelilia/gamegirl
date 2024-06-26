// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

mod cop0;
mod inst;

use std::marker::ConstParamTy;

use common::numutil::NumExt;

use crate::{cpu::cop0::Cop0, PlayStation};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cpu {
    regs: [u32; 32],
    next_regs: [u32; 32],
    pending_load: PendingLoad,

    pub pc: u32,
    next_pc: u32,
    current_pc: u32,
    pipeline: u32,

    is_branch: bool,
    is_delay: bool,

    cop0: Cop0,
    pub hi: u32,
    pub lo: u32,
}

impl Cpu {
    pub fn execute_next(ps: &mut PlayStation) {
        if !ps.debugger.should_execute(ps.cpu.pc) {
            return;
        }

        ps.cpu
            .set_reg(ps.cpu.pending_load.reg, ps.cpu.pending_load.value);
        ps.cpu.pending_load = PendingLoad::default();

        ps.cpu.current_pc = ps.cpu.pc;
        ps.cpu.pc = ps.cpu.next_pc;
        ps.cpu.next_pc = ps.cpu.next_pc.wrapping_add(4);
        Cpu::ensure_aligned(ps, ps.cpu.pc, 4, Exception::UnalignedLoad);

        ps.cpu.is_delay = ps.cpu.is_branch;
        ps.cpu.is_branch = false;

        let inst = ps.get(ps.cpu.current_pc);
        ps.run_inst(inst);

        // Do not overwrite zero register
        ps.cpu.regs[1..].copy_from_slice(&ps.cpu.next_regs[1..]);

        // TODO timing
        ps.advance_clock(2);
    }

    pub fn reg(&self, idx: u32) -> u32 {
        self.regs[idx.us()]
    }

    fn set_reg(&mut self, idx: u32, value: u32) {
        self.next_regs[idx.us()] = value;
    }

    fn ensure_aligned(ps: &mut PlayStation, addr: u32, by: u32, exception: Exception) {
        if addr & (by - 1) != 0 {
            Cpu::exception_occurred(ps, exception);
        }
    }

    fn exception_occurred(ps: &mut PlayStation, kind: Exception) {
        let new_pc = if ps.cpu.cop0.sr.is_bit(22) {
            0xBFC0_0180
        } else {
            0x8000_0080
        };

        let context = ps.cpu.cop0.sr & 0x3F;
        ps.cpu.cop0.sr &= !0x3F;
        ps.cpu.cop0.sr |= (context << 2) & 0x3F;
        ps.cpu.cop0.cause = (kind as u32) << 2;
        ps.cpu.cop0.epc = ps.cpu.current_pc;

        if ps.cpu.is_delay {
            ps.cpu.cop0.epc = ps.cpu.current_pc.wrapping_sub(4);
            ps.cpu.cop0.cause = ps.cpu.cop0.cause.set_bit(31, true);
        } else {
            ps.cpu.cop0.epc = ps.cpu.current_pc;
        }

        ps.cpu.pc = new_pc;
        ps.cpu.pc = new_pc + 4;
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            regs: [0; 32],
            next_regs: [0; 32],
            pending_load: PendingLoad::default(),
            pc: 0xBFC0_0000,
            next_pc: 0xBFC0_0004,
            current_pc: 0xBFC0_0000,
            pipeline: 0,

            is_branch: false,
            is_delay: false,

            cop0: Cop0::default(),
            hi: 0,
            lo: 0,
        }
    }
}

impl PlayStation {
    fn jump_pc(&mut self, value: u32) {
        self.cpu.next_pc = value;
        self.cpu.is_branch = true;
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct PendingLoad {
    reg: u32,
    value: u32,
}

#[derive(Eq, PartialEq, ConstParamTy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
enum Exception {
    UnalignedLoad = 0x4,
    UnalignedStore = 0x5,
    Syscall = 0x8,
    Break = 0x9,
    UnknownOpcode = 0xA,
    CopError = 0xB,
    Overflow = 0xC,
}
