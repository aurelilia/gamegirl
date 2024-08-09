// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![allow(incomplete_features)]
#![feature(adt_const_params)]
#![feature(const_mut_refs)]

pub mod arm;
mod exceptions;
pub mod interface;
mod misc;
mod optimizations;
pub mod registers;
mod thumb;

use std::fmt::Write;

use access::{CODE, NONSEQ, SEQ};
use common::numutil::NumExt;
pub use exceptions::*;
use optimizations::waitloop::WaitloopData;
use registers::Flag;

use crate::{
    arm::ArmInst,
    interface::{ArmSystem, RwType, SysWrapper},
    optimizations::caching::{Cache, CacheEntry, CachedInst},
    registers::{FiqReg, Flag::Thumb, ModeReg},
    thumb::ThumbInst,
};

/// Represents the CPU of the console - an ARM7TDMI.
/// It is generic over the system used; see `interface.rs`.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cpu<S: ArmSystem + 'static> {
    pub fiqs: [FiqReg; 5],
    pub sp: ModeReg,
    pub lr: ModeReg,
    pub cpsr: u32,
    pub spsr: ModeReg,

    pub registers: [u32; 16],
    pub pipeline: [u32; 2],
    pub access_type: Access,
    pub is_halted: bool,

    pub ime: bool,
    pub ie: u32,
    pub if_: u32,

    block_ended: bool,
    pipeline_valid: bool,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    pub cache: Cache<S>,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    waitloop: WaitloopData,
}

impl<S: ArmSystem> Cpu<S> {
    /// Advance emulation.
    #[inline]
    pub fn continue_running(gg: &mut S) {
        let pc = gg.cpur().pc();
        if !gg.debugger().should_execute(pc) {
            return;
        }

        let gg = SysWrapper::new(gg);
        if gg.cpu().cache.enabled {
            if let Some(cache) = gg.cpu().cache.get(pc) {
                Cpu::run_cache(gg, cache);
                return;
            } else if pc < 0x1000_0000 {
                Cpu::try_make_cache(gg);
                return;
            }
        }
        Self::execute_next_inst(gg);
    }

    /// Execute the next instruction and advance the scheduler.
    fn execute_next_inst(gg: &mut SysWrapper<S>) {
        gg.advance_clock();
        Self::ensure_pipeline_valid(gg);
        if gg.cpu().flag(Thumb) {
            let (inst, _, pc) = Self::fetch_next_inst::<u16>(gg);
            gg.will_execute(pc);
            gg.execute_thumb(inst.u16());
        } else {
            let (inst, _, pc) = Self::fetch_next_inst::<u32>(gg);
            gg.will_execute(pc);
            gg.execute_inst_arm(inst);
        }
    }

    /// Run the given cache block for as long as possible.
    fn run_cache(gg: &mut SysWrapper<S>, cache: CacheEntry<S>) {
        gg.cpu().block_ended = false;
        gg.cpu().pipeline_valid = false;

        let is_thumb = gg.cpu().flag(Thumb);
        match cache {
            CacheEntry::Arm(cache) if !is_thumb => {
                for inst in cache.iter() {
                    gg.advance_clock();
                    if gg.cpu().block_ended {
                        // CPU got interrupted, stop
                        return;
                    }

                    let pc = gg.cpu().inc_pc_by(4);
                    gg.will_execute(pc);
                    gg.add_sn_cycles(inst.sn_cycles);
                    if gg.check_arm_cond(inst.inst) {
                        (inst.handler)(gg, ArmInst(inst.inst));
                    }
                }
            }
            CacheEntry::Thumb(cache) if is_thumb => {
                for inst in cache.iter() {
                    gg.advance_clock();
                    if gg.cpu().block_ended {
                        // CPU got interrupted, stop
                        return;
                    }

                    let pc = gg.cpu().inc_pc_by(2);
                    gg.will_execute(pc);
                    gg.add_sn_cycles(inst.sn_cycles);
                    (inst.handler)(gg, ThumbInst::of(inst.inst));
                }
            }

            // Edge case: Somehow we got a cache entry that doesn't match the CPU state
            // Just advance regularly and ignore the cache
            // (The only game known to me that triggers this is "Hello Kitty - Happy Party Pals")
            _ => Self::execute_next_inst(gg),
        }
    }

    /// Try to make a cache block at the current location.
    /// If we get interrrupted by an IRQ, will abort to ensure
    /// cache blocks are as long as possible.
    fn try_make_cache(gg: &mut SysWrapper<S>) {
        let start_pc = gg.cpu().pc();
        gg.cpu().block_ended = false;
        if gg.cpu().flag(Thumb) {
            let mut block = Vec::with_capacity(5);
            while !gg.cpu().block_ended {
                gg.advance_clock();
                Self::ensure_pipeline_valid(gg);
                if gg.cpu().block_ended {
                    // CPU got interrupted by system, discard the block
                    return;
                }

                let (inst, sn_cycles, pc) = Self::fetch_next_inst::<u16>(gg);
                gg.will_execute(pc);
                let inst = inst.u16();
                let handler = SysWrapper::<S>::get_handler_thumb(inst);

                handler(gg, ThumbInst::of(inst));
                block.push(CachedInst {
                    inst,
                    handler,
                    sn_cycles,
                });
                if Cache::<S>::force_end_block(gg.cpu().pc()) {
                    // Block is in IWRAM and hit a page boundary, finish the block
                    break;
                }
            }
            gg.cpu()
                .cache
                .put(start_pc, CacheEntry::Thumb(Box::leak(Box::new(block))));
        } else {
            let mut block = Vec::with_capacity(5);
            while !gg.cpu().block_ended {
                gg.advance_clock();
                Self::ensure_pipeline_valid(gg);
                if gg.cpu().block_ended {
                    // CPU got interrupted by system, discard the block
                    return;
                }

                let (inst, sn_cycles, pc) = Self::fetch_next_inst::<u32>(gg);
                gg.will_execute(pc);
                let handler = SysWrapper::<S>::get_handler_arm(inst);

                if gg.check_arm_cond(inst) {
                    handler(gg, ArmInst(inst));
                }
                block.push(CachedInst {
                    inst,
                    handler,
                    sn_cycles,
                });
                if Cache::<S>::force_end_block(gg.cpu().pc()) {
                    // Block is in IWRAM and hit a page boundary, finish the block
                    break;
                }
            }
            gg.cpu()
                .cache
                .put(start_pc, CacheEntry::Arm(Box::leak(Box::new(block))));
        }
    }

    /// Fetch the next instruction of the CPU.
    fn fetch_next_inst<TY: RwType>(gg: &mut S) -> (u32, u16, u32) {
        let pc = gg.cpu().inc_pc_by(TY::WIDTH);
        let sn_cycles = gg.wait_time::<TY>(gg.cpur().pc(), gg.cpur().access_type | CODE);
        gg.add_sn_cycles(sn_cycles);

        let inst = gg.cpu().pipeline[0];
        gg.cpu().pipeline[0] = gg.cpu().pipeline[1];
        gg.cpu().pipeline[1] = gg.get::<TY>(gg.cpur().pc()).u32();
        gg.cpu().access_type = SEQ;

        Self::trace_inst::<TY>(gg, inst);
        (inst, sn_cycles, pc)
    }

    fn trace_inst<TY: NumExt + 'static>(gg: &mut S, inst: u32) {
        if gg.debugger().tracing() {
            let cpsr = gg.cpu().cpsr;
            let mnem = if TY::WIDTH == 2 {
                ThumbInst::of(inst.u16()).to_string()
            } else {
                Self::get_mnemonic_arm(inst)
            };

            let mut buf = String::with_capacity(100);
            let num = ('4' as u8 + S::IS_V5 as u8) as char;
            buf.push(num);
            for reg in gg.cpu().registers.iter().enumerate() {
                let reg = reg.1;
                write!(buf, "{reg:08X} ").ok();
            }

            if TY::WIDTH == 2 {
                gg.debugger().add_traced_instruction(|| {
                    format!("{buf}cpsr: {cpsr:08X} |     {inst:04X}: {mnem}")
                });
            } else {
                gg.debugger().add_traced_instruction(|| {
                    format!("{buf}cpsr: {cpsr:08X} | {inst:08X}: {mnem}")
                });
            }
        }
    }

    pub fn get_inst(gg: &mut S, ptr: u32) -> String {
        if gg.cpur().flag(Flag::Thumb) {
            let inst = gg.get(ptr);
            ThumbInst::of(inst).to_string()
        } else {
            let inst = gg.get(ptr);
            Cpu::<S>::get_mnemonic_arm(inst)
        }
    }

    /// Emulate a pipeline stall / fill; used when PC changes.
    pub fn pipeline_stall(gg: &mut S) {
        gg.pipeline_stalled();
        if gg.cpu().flag(Thumb) {
            let time = gg.wait_time::<u16>(gg.cpur().pc(), NONSEQ | CODE);
            gg.add_sn_cycles(time);
            gg.cpu().inc_pc_by(2);
            let time = gg.wait_time::<u16>(gg.cpur().pc(), SEQ | CODE);
            gg.add_sn_cycles(time);
        } else {
            let time = gg.wait_time::<u32>(gg.cpur().pc(), NONSEQ | CODE);
            gg.add_sn_cycles(time);
            gg.cpu().inc_pc_by(4);
            let time = gg.wait_time::<u32>(gg.cpur().pc(), SEQ | CODE);
            gg.add_sn_cycles(time);
        };
        gg.cpu().access_type = SEQ;
        gg.cpu().block_ended = true;
        gg.cpu().pipeline_valid = false;
    }

    /// Ensure the pipeline is valid, which it might not be after
    /// a cache block was executed.
    fn ensure_pipeline_valid(gg: &mut S) {
        if gg.cpu().pipeline_valid {
            return;
        }
        if gg.cpu().flag(Thumb) {
            gg.cpu().pipeline[0] = gg.get::<u16>(gg.cpur().pc() - 2).u32();
            gg.cpu().pipeline[1] = gg.get::<u16>(gg.cpur().pc()).u32();
        } else {
            gg.cpu().pipeline[0] = gg.get::<u32>(gg.cpur().pc() - 4);
            gg.cpu().pipeline[1] = gg.get::<u32>(gg.cpur().pc());
        }
        gg.cpu().pipeline_valid = true;
    }

    #[inline]
    fn inc_pc_by(&mut self, count: u32) -> u32 {
        self.registers[15] = self.registers[15].wrapping_add(count);
        self.registers[15]
    }

    #[inline]
    pub fn inst_size(&self) -> u32 {
        // 4 on ARM, 2 on THUMB
        4 - ((self.flag(Thumb) as u32) << 1)
    }
}

impl<S: ArmSystem> Default for Cpu<S> {
    fn default() -> Self {
        Self {
            fiqs: [FiqReg::default(); 5],
            sp: [0x0300_7F00, 0x0, 0x0300_7FE0, 0x0, 0x0300_7FA0, 0x0],
            lr: ModeReg::default(),
            cpsr: 0xD3,
            spsr: ModeReg::default(),
            registers: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4],
            pipeline: [0; 2],
            access_type: NONSEQ,
            is_halted: false,
            block_ended: false,
            pipeline_valid: false,
            cache: Cache::default(),
            waitloop: WaitloopData::default(),

            ime: false,
            ie: 0,
            if_: 0,
        }
    }
}

/// Enum for the types of memory accesses; either sequential
/// or non-sequential. The numbers assigned to the variants are
/// to speed up reading the wait times in `memory.rs`.
pub type Access = u8;

pub mod access {
    use crate::Access;
    pub const NONSEQ: Access = 0;
    pub const SEQ: Access = 1 << 0;
    pub const CODE: Access = 1 << 1;
    pub const DMA: Access = 1 << 2;
}
