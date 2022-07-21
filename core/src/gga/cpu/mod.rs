// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod alu;
mod caching;
mod inst_arm;
mod inst_generic;
mod inst_thumb;
pub mod registers;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    gga::{
        addr::*,
        cpu::{
            caching::{Cache, CacheEntry, CachedInst},
            inst_arm::ArmInst,
            inst_thumb::ThumbInst,
            registers::{
                FiqReg,
                Flag::{FiqDisable, IrqDisable, Thumb},
                Mode, ModeReg,
            },
        },
        Access, GameGirlAdv,
    },
    numutil::NumExt,
};

/// Represents the CPU of the console - an ARM7TDMI.
#[derive(Deserialize, Serialize)]
pub struct Cpu {
    pub fiqs: [FiqReg; 5],
    pub sp: ModeReg,
    pub lr: ModeReg,
    pub cpsr: u32,
    pub spsr: ModeReg,

    pub registers: [u32; 16],
    pipeline: [u32; 2],
    pub(crate) access_type: Access,

    block_ended: bool,
    pipeline_valid: bool,
    #[serde(default)]
    #[serde(skip)]
    pub(crate) cache: Cache,

    #[cfg_attr(feature = "instruction-tracing", serde(default))]
    #[cfg_attr(feature = "instruction-tracing", serde(skip))]
    #[cfg(feature = "instruction-tracing")]
    pub instruction_tracer: Option<Box<dyn Fn(&GameGirlAdv, u32) + Send + 'static>>,
}

impl Cpu {
    /// Advance emulation..
    pub fn continue_running(gg: &mut GameGirlAdv) {
        if !gg.debugger.should_execute(gg.cpu.pc()) {
            gg.options.running = false; // Pause emulation, we hit a BP
            return;
        }

        if gg.config.cached_interpreter {
            if let Some(cache) = gg.cpu.cache.get(gg.cpu.pc()) {
                Cpu::run_cache(gg, cache);
                return;
            } else if Cache::can_make_cache(gg.cpu.pc()) {
                Cpu::try_make_cache(gg);
                return;
            }
        }
        Self::execute_next_inst(gg);
    }

    /// Execute the next instruction and advance the scheduler.
    fn execute_next_inst(gg: &mut GameGirlAdv) {
        gg.advance_clock();
        Self::ensure_pipeline_valid(gg);
        if gg.cpu.flag(Thumb) {
            let (inst, _) = Self::fetch_next_inst::<2, true>(gg);
            gg.execute_inst_thumb(inst.u16());
        } else {
            let (inst, _) = Self::fetch_next_inst::<4, false>(gg);
            gg.execute_inst_arm(inst);
        }
    }

    fn run_cache(gg: &mut GameGirlAdv, cache: CacheEntry) {
        gg.cpu.block_ended = false;
        gg.cpu.pipeline_valid = false;

        match cache {
            CacheEntry::Arm(cache) => {
                assert!(!gg.cpu.flag(Thumb));
                for inst in cache.iter() {
                    gg.advance_clock();
                    if gg.cpu.block_ended {
                        // CPU got interrupted, stop
                        return;
                    }

                    gg.cpu.inc_pc_by(4);
                    Self::trace_inst::<false>(gg, inst.inst);
                    gg.add_sn_cycles(inst.sn_cycles);
                    if gg.check_arm_cond(inst.inst) {
                        (inst.handler)(gg, ArmInst(inst.inst));
                    }
                }
            }
            CacheEntry::Thumb(cache) => {
                assert!(gg.cpu.flag(Thumb));
                for inst in cache.iter() {
                    gg.advance_clock();
                    if gg.cpu.block_ended {
                        // CPU got interrupted, stop
                        return;
                    }

                    gg.cpu.inc_pc_by(2);
                    Self::trace_inst::<true>(gg, inst.inst.u32());
                    gg.add_sn_cycles(inst.sn_cycles);
                    (inst.handler)(gg, ThumbInst(inst.inst));
                }
            }
        }
    }

    fn try_make_cache(gg: &mut GameGirlAdv) {
        let start_pc = gg.cpu.pc();
        gg.cpu.block_ended = false;
        if gg.cpu.flag(Thumb) {
            let mut block = Vec::with_capacity(5);
            while !gg.cpu.block_ended {
                gg.advance_clock();
                Self::ensure_pipeline_valid(gg);
                if gg.cpu.block_ended {
                    // CPU got interrupted by system, discard the block
                    return;
                }

                let (inst, sn_cycles) = Self::fetch_next_inst::<2, true>(gg);
                let inst = inst.u16();
                let handler = GameGirlAdv::get_handler_thumb(inst);

                handler(gg, ThumbInst(inst));
                block.push(CachedInst {
                    inst,
                    handler,
                    sn_cycles,
                });
                if Cache::force_end_block(gg.cpu.pc()) {
                    // Block is in IWRAM and hit a page boundary, finish the block
                    break;
                }
            }
            gg.cpu
                .cache
                .put(start_pc, CacheEntry::Thumb(Arc::new(block)));
        } else {
            let mut block = Vec::with_capacity(5);
            while !gg.cpu.block_ended {
                gg.advance_clock();
                Self::ensure_pipeline_valid(gg);
                if gg.cpu.block_ended {
                    // CPU got interrupted by system, discard the block
                    return;
                }

                let (inst, sn_cycles) = Self::fetch_next_inst::<4, false>(gg);
                let handler = GameGirlAdv::get_handler_arm(inst);

                if gg.check_arm_cond(inst) {
                    handler(gg, ArmInst(inst));
                }
                block.push(CachedInst {
                    inst,
                    handler,
                    sn_cycles,
                });
                if Cache::force_end_block(gg.cpu.pc()) {
                    // Block is in IWRAM and hit a page boundary, finish the block
                    break;
                }
            }
            gg.cpu.cache.put(start_pc, CacheEntry::Arm(Arc::new(block)));
        }
    }

    fn fetch_next_inst<const WAIT: u32, const THUMB: bool>(gg: &mut GameGirlAdv) -> (u32, u16) {
        gg.cpu.inc_pc_by(WAIT);
        let sn_cycles = gg.wait_time::<WAIT>(gg.cpu.pc(), gg.cpu.access_type);
        gg.add_sn_cycles(sn_cycles);

        let inst = gg.cpu.pipeline[0];
        gg.cpu.pipeline[0] = gg.cpu.pipeline[1];
        gg.cpu.pipeline[1] = if THUMB {
            gg.get_hword(gg.cpu.pc()).u32()
        } else {
            gg.get_word(gg.cpu.pc())
        };
        gg.cpu.access_type = Access::Seq;

        Self::trace_inst::<THUMB>(gg, inst);
        (inst, sn_cycles)
    }

    fn trace_inst<const THUMB: bool>(gg: &mut GameGirlAdv, inst: u32) {
        if crate::TRACING {
            let mnem = if THUMB {
                GameGirlAdv::get_mnemonic_thumb(inst.u16())
            } else {
                GameGirlAdv::get_mnemonic_arm(inst)
            };
            eprintln!("0x{:08X} {}", gg.cpu.pc(), mnem);
        }

        #[cfg(feature = "instruction-tracing")]
        if let Some(tracer) = &gg.cpu.instruction_tracer {
            tracer(gg, inst);
        }
    }

    /// Check if an interrupt needs to be handled and jump to the handler if so.
    /// Called on any events that might cause an interrupt to be triggered..
    pub fn check_if_interrupt(gg: &mut GameGirlAdv) {
        let int = (gg[IME] == 1) && !gg.cpu.flag(IrqDisable) && (gg[IE] & gg[IF]) != 0;
        if int {
            gg.cpu.inc_pc_by(4);
            Cpu::exception_occurred(gg, Exception::Irq);
        }
    }

    /// An exception occurred, jump to the bootrom handler and deal with it.
    fn exception_occurred(gg: &mut GameGirlAdv, kind: Exception) {
        if gg.cpu.pc() > 0x100_0000 {
            gg.memory.bios_value = 0xE25E_F004;
        }
        if gg.cpu.flag(Thumb) {
            gg.cpu.inc_pc_by(2); // ??
        }

        let cpsr = gg.cpu.cpsr;
        gg.cpu.set_mode(kind.mode());

        gg.cpu.set_flag(Thumb, false);
        gg.cpu.set_flag(IrqDisable, true);
        if let Exception::Reset | Exception::Fiq = kind {
            gg.cpu.set_flag(FiqDisable, true);
        }

        gg.cpu.set_lr(gg.cpu.pc() - gg.cpu.inst_size());
        gg.cpu.set_spsr(cpsr);
        gg.set_pc(kind.vector());
    }

    /// Emulate a pipeline stall / fill; used when PC changes.
    pub fn pipeline_stall(gg: &mut GameGirlAdv) {
        gg.memory.prefetch_len = 0; // Discard prefetch
        if gg.cpu.flag(Thumb) {
            let time = gg.wait_time::<2>(gg.cpu.pc(), Access::NonSeq);
            gg.add_sn_cycles(time);
            gg.cpu.inc_pc_by(2);
            let time = gg.wait_time::<2>(gg.cpu.pc(), Access::Seq);
            gg.add_sn_cycles(time);
        } else {
            let time = gg.wait_time::<4>(gg.cpu.pc(), Access::NonSeq);
            gg.add_sn_cycles(time);
            gg.cpu.inc_pc_by(4);
            let time = gg.wait_time::<4>(gg.cpu.pc(), Access::Seq);
            gg.add_sn_cycles(time);
        };
        gg.cpu.access_type = Access::Seq;
        gg.cpu.block_ended = true;
        gg.cpu.pipeline_valid = false;
    }

    fn ensure_pipeline_valid(gg: &mut GameGirlAdv) {
        if gg.cpu.pipeline_valid {
            return;
        }
        if gg.cpu.flag(Thumb) {
            gg.cpu.pipeline[0] = gg.get_hword(gg.cpu.pc() - 2).u32();
            gg.cpu.pipeline[1] = gg.get_hword(gg.cpu.pc()).u32();
        } else {
            gg.cpu.pipeline[0] = gg.get_word(gg.cpu.pc() - 4);
            gg.cpu.pipeline[1] = gg.get_word(gg.cpu.pc());
        }
        gg.cpu.pipeline_valid = true;
    }

    #[inline]
    fn inc_pc_by(&mut self, count: u32) {
        self.registers[15] = self.registers[15].wrapping_add(count);
    }

    #[inline]
    pub fn inst_size(&self) -> u32 {
        // 4 on ARM, 2 on THUMB
        4 - ((self.flag(Thumb) as u32) << 1)
    }

    /// Request an interrupt. Will check if the CPU will service it right away.
    #[inline]
    pub fn request_interrupt(gg: &mut GameGirlAdv, int: Interrupt) {
        Self::request_interrupt_idx(gg, int as u16);
    }

    /// Request an interrupt by index. Will check if the CPU will service it
    /// right away.
    #[inline]
    pub fn request_interrupt_idx(gg: &mut GameGirlAdv, idx: u16) {
        gg[IF] = gg[IF].set_bit(idx, true);
        Self::check_if_interrupt(gg);
    }
}

impl Default for Cpu {
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
            block_ended: false,
            pipeline_valid: false,
            cache: Cache::default(),

            #[cfg(feature = "instruction-tracing")]
            instruction_tracer: None,
        }
    }
}

/// Possible interrupts.
#[repr(C)]
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
/// Most are only listed to preserve bit order in IE/IF, only SWI
/// and IRQ ever get raised on the GGA. (UND does as well, but this
/// emulator doesn't implement that.)
#[derive(Copy, Clone)]
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
