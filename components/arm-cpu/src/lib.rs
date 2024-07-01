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

mod alu;
mod caching;
pub mod inst_arm;
mod inst_generic;
pub mod inst_thumb;
pub mod interface;
mod lut;
pub mod registers;

use std::sync::Arc;

use common::{components::debugger::Severity, numutil::NumExt};

use crate::{
    caching::{Cache, CacheEntry, CachedInst},
    inst_arm::ArmInst,
    inst_thumb::ThumbInst,
    interface::{ArmSystem, RwType, SysWrapper},
    registers::{
        FiqReg,
        Flag::{FiqDisable, IrqDisable, Thumb},
        Mode, ModeReg,
    },
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
    pipeline: [u32; 2],
    pub access_type: Access,
    pub is_halted: bool,

    pub ime: bool,
    pub ie: u32,
    pub if_: u32,

    block_ended: bool,
    pipeline_valid: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub cache: Cache<S>,
}

impl<S: ArmSystem> Cpu<S> {
    /// Advance emulation.
    #[inline]
    pub fn continue_running(gg: &mut S) {
        let pc = gg.cpur().pc();
        if !gg.debugger().should_execute(pc) {
            return;
        }

        let mut wrapper = SysWrapper {
            inner: gg as *mut S,
        };
        if gg.cpu().cache.enabled {
            if let Some(cache) = gg.cpu().cache.get(pc) {
                Cpu::run_cache(&mut wrapper, cache);
                return;
            } else if S::can_cache_at(pc) {
                Cpu::try_make_cache(&mut wrapper);
                return;
            }
        }
        Self::execute_next_inst(&mut wrapper);
    }

    /// Execute the next instruction and advance the scheduler.
    fn execute_next_inst(gg: &mut SysWrapper<S>) {
        gg.advance_clock();
        Self::ensure_pipeline_valid(gg);
        if gg.cpu().flag(Thumb) {
            let (inst, _, pc) = Self::fetch_next_inst::<u16>(gg);
            gg.will_execute(pc);
            gg.execute_inst_thumb(inst.u16());
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

        match cache {
            CacheEntry::Arm(cache) => {
                assert!(!gg.cpu().flag(Thumb));
                for inst in cache.iter() {
                    gg.advance_clock();
                    if gg.cpu().block_ended {
                        // CPU got interrupted, stop
                        return;
                    }

                    let pc = gg.cpu().inc_pc_by(4);
                    gg.will_execute(pc);
                    Self::trace_inst::<u32>(gg, inst.inst);
                    gg.add_sn_cycles(inst.sn_cycles);
                    if gg.check_arm_cond(inst.inst) {
                        (inst.handler)(gg, ArmInst(inst.inst));
                    }
                }
            }
            CacheEntry::Thumb(cache) => {
                assert!(gg.cpu().flag(Thumb));
                for inst in cache.iter() {
                    gg.advance_clock();
                    if gg.cpu().block_ended {
                        // CPU got interrupted, stop
                        return;
                    }

                    let pc = gg.cpu().inc_pc_by(2);
                    gg.will_execute(pc);
                    Self::trace_inst::<u16>(gg, inst.inst.u32());
                    gg.add_sn_cycles(inst.sn_cycles);
                    (inst.handler)(gg, ThumbInst(inst.inst));
                }
            }
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

                handler(gg, ThumbInst(inst));
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
                .put(start_pc, CacheEntry::Thumb(Arc::new(block)));
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
                .put(start_pc, CacheEntry::Arm(Arc::new(block)));
        }
    }

    /// Fetch the next instruction of the CPU.
    fn fetch_next_inst<TY: RwType>(gg: &mut S) -> (u32, u16, u32) {
        let pc = gg.cpu().inc_pc_by(TY::WIDTH);
        let sn_cycles = gg.wait_time::<TY>(gg.cpur().pc(), gg.cpur().access_type);
        gg.add_sn_cycles(sn_cycles);

        let inst = gg.cpu().pipeline[0];
        gg.cpu().pipeline[0] = gg.cpu().pipeline[1];
        gg.cpu().pipeline[1] = gg.get::<TY>(gg.cpur().pc()).u32();
        gg.cpu().access_type = Access::Seq;

        Self::trace_inst::<TY>(gg, inst);
        (inst, sn_cycles, pc)
    }

    fn trace_inst<TY: NumExt + 'static>(gg: &mut S, inst: u32) {
        if gg.debugger().tracing() {
            let pc = gg.cpu().pc();
            gg.debugger().add_traced_instruction(|| {
                let mnem = if TY::WIDTH == 2 {
                    Self::get_mnemonic_thumb(inst.u16())
                } else {
                    Self::get_mnemonic_arm(inst)
                };
                format!("0x{:08X} {}", pc, mnem)
            });
        }
    }

    /// Check if an interrupt needs to be handled and jump to the handler if so.
    /// Called on any events that might cause an interrupt to be triggered..
    pub fn check_if_interrupt(gg: &mut S) {
        if gg.cpur().is_interrupt_pending() {
            gg.cpu().inc_pc_by(4);
            let mut wrapper = SysWrapper {
                inner: gg as *mut S,
            };
            Cpu::exception_occurred(&mut wrapper, Exception::Irq);
        }
    }

    fn is_interrupt_pending(&self) -> bool {
        self.ime && !self.flag(IrqDisable) && (self.ie & self.if_) != 0
    }

    /// An exception occurred, jump to the bootrom handler and deal with it.
    fn exception_occurred(gg: &mut SysWrapper<S>, kind: Exception) {
        gg.exception_happened(kind);
        if gg.cpu().flag(Thumb) {
            gg.cpu().inc_pc_by(2); // ??
        }

        let cpsr = gg.cpu().cpsr;
        gg.cpu().set_mode(kind.mode());

        gg.cpu().set_flag(Thumb, false);
        gg.cpu().set_flag(IrqDisable, true);
        if let Exception::Reset | Exception::Fiq = kind {
            gg.debugger().log(
                "exception-raised",
                format!("An unusual exception got raised: {kind:?}"),
                Severity::Warning,
            );
            gg.cpu().set_flag(FiqDisable, true);
        }

        let lr = gg.cpur().pc() - gg.cpur().inst_size();
        gg.cpu().set_lr(lr);
        gg.cpu().set_spsr(cpsr);
        gg.set_pc(kind.vector());
    }

    /// Emulate a pipeline stall / fill; used when PC changes.
    pub fn pipeline_stall(gg: &mut S) {
        // gg.memory.prefetch_len = 0; // Discard prefetch
        gg.pipeline_stalled();
        if gg.cpu().flag(Thumb) {
            let time = gg.wait_time::<u16>(gg.cpur().pc(), Access::NonSeq);
            gg.add_sn_cycles(time);
            gg.cpu().inc_pc_by(2);
            let time = gg.wait_time::<u16>(gg.cpur().pc(), Access::Seq);
            gg.add_sn_cycles(time);
        } else {
            let time = gg.wait_time::<u32>(gg.cpur().pc(), Access::NonSeq);
            gg.add_sn_cycles(time);
            gg.cpu().inc_pc_by(4);
            let time = gg.wait_time::<u32>(gg.cpur().pc(), Access::Seq);
            gg.add_sn_cycles(time);
        };
        gg.cpu().access_type = Access::Seq;
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

    /// Request an interrupt. Will check if the CPU will service it right away.
    #[inline]
    pub fn request_interrupt(gg: &mut S, int: Interrupt) {
        Self::request_interrupt_idx(gg, int as u16);
    }

    /// Request an interrupt by index. Will check if the CPU will service it
    /// right away.
    #[inline]
    pub fn request_interrupt_idx(gg: &mut S, idx: u16) {
        let me = gg.cpu();
        me.if_ = me.if_.set_bit(idx, true);
        Self::check_if_interrupt(gg);
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
            access_type: Access::NonSeq,
            is_halted: false,
            block_ended: false,
            pipeline_valid: false,
            cache: Cache::default(),

            ime: false,
            ie: 0,
            if_: 0,
        }
    }
}

/// Possible interrupts.
/// These are the same between GGA and NDS, so
/// putting them here is OK.
#[repr(C)]
#[derive(Copy, Clone)]
pub enum Interrupt {
    VBlank,
    HBlank,
    VCounter,
    Timer0,
    Timer1,
    Timer2,
    Timer3,
    Serial,
    Dma0,
    Dma1,
    Dma2,
    Dma3,
    Joypad,
    GamePak,
}

/// Possible exceptions.
/// Most are only listed to preserve bit order in IE/IF, only SWI, UND
/// and IRQ ever get raised on the GGA.
#[derive(Debug, Copy, Clone)]
pub enum Exception {
    Reset,
    Undefined,
    Swi,
    PrefetchAbort,
    DataAbort,
    AddressExceeded,
    Irq,
    Fiq,
}

impl Exception {
    /// Vector to set the PC to when this exception occurs.
    fn vector(self) -> u32 {
        self as u32 * 4
    }

    /// Mode to execute the exception in.
    fn mode(self) -> Mode {
        const MODE: [Mode; 8] = [
            Mode::Supervisor,
            Mode::Undefined,
            Mode::Supervisor,
            Mode::Abort,
            Mode::Abort,
            Mode::Supervisor,
            Mode::Irq,
            Mode::Fiq,
        ];
        MODE[self as usize]
    }
}

/// Enum for the types of memory accesses; either sequential
/// or non-sequential. The numbers assigned to the variants are
/// to speed up reading the wait times in `memory.rs`.
#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Access {
    Seq = 0,
    NonSeq = 16,
}
