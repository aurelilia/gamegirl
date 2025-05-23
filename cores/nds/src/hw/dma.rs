// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use armchair::{
    access::{DMA, NONSEQ, SEQ},
    interface::RwType,
    Access, Address, Cpu, Interrupt,
};
use arrayvec::ArrayVec;
use common::numutil::{word, NumExt};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use crate::{addr::VCOUNT, io::IoSection, NdsCpu};

const SRC_MASK_7: [u32; 4] = [0x7FF_FFFF, 0xFFF_FFFF, 0xFFF_FFFF, 0xFFF_FFFF];
const DST_MASK_7: [u32; 4] = [0x7FF_FFFF, 0x7FF_FFFF, 0x7FF_FFFF, 0xFFF_FFFF];

#[derive(Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Dma {
    pub sad: u32,
    pub dad: u32,
    pub count: u16,
    pub ctrl: DmaControl,

    /// Internal source register
    src: u32,
    /// Internal destination register
    dst: u32,
}

/// NDS's 8 DMA channels, separated by CPU.
/// TODO: Fill registers
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Dmas {
    pub channels: [Dma; 4],
    /// Internal cache shared between DMAs
    pub(super) cache: u32,
    /// Currently running DMA, or 99
    pub(super) running: u16,
    /// DMAs waiting to run after current.
    queued: ArrayVec<(u16, Reason), 3>,
    /// PC when the last DMA finished (for lingering bus behavior)
    pub(super) pc_at_last_end: u32,
}

impl Dmas {
    /// Update all DMAs to see if they need ticking.
    pub fn update_all(ds: &mut impl NdsCpu, reason: Reason) {
        for idx in 0..4 {
            Self::step_dma(ds, idx, reason);
        }
    }

    /// Update a given DMA after it's control register was written.
    pub fn ctrl_write<DS: NdsCpu>(ds: &mut DS, idx: usize, new_ctrl: IoSection<u16>) {
        let channel = &mut ds.dmas[DS::I].channels[idx];
        let old_ctrl = channel.ctrl;
        let mut new_ctrl = new_ctrl.apply_io_ret(&mut channel.ctrl);

        if !old_ctrl.dma_en() && new_ctrl.dma_en() {
            // Reload SRC/DST
            if DS::I == 0 {
                channel.src = channel.sad & SRC_MASK_7[idx];
                channel.dst = channel.dad & DST_MASK_7[idx];
            } else {
                channel.src = channel.sad;
                channel.dst = channel.dad;
            }
        }

        Self::step_dma(ds, idx, Reason::CtrlWrite);
    }

    /// Step a DMA and perform a transfer if possible.
    fn step_dma<DS: NdsCpu>(ds: &mut DS, idx: usize, reason: Reason) {
        let mut channel = ds.dmas[DS::I].channels[idx];
        let ctrl = channel.ctrl;

        let on = ctrl.dma_en()
            && if DS::I == 0 {
                // NDS7
                match ctrl.timing() {
                    Timing::Now => reason == Reason::CtrlWrite,
                    Timing::VBlank => reason == Reason::VBlank,
                    Timing::HBlank => reason == Reason::CartridgeReady,
                    Timing::Special => false, // TODO wireless?
                }
            } else {
                // NDS9
                match (ctrl.timing_ext(), ctrl.timing()) {
                    (false, Timing::Now) => reason == Reason::CtrlWrite,
                    (true, Timing::Now) => reason == Reason::VBlank,
                    (false, Timing::VBlank) => reason == Reason::HBlank && ds.gpu.vcount < 160,
                    (true, Timing::VBlank) => reason == Reason::HBlank && ds.gpu.vcount == 0,
                    (false, Timing::HBlank) => false, // TODO main memory display
                    (true, Timing::HBlank) => reason == Reason::CartridgeReady, // DS cart
                    (false, Timing::Special) => false, // GBA cart
                    (true, Timing::Special) => false, // TODO geometry FIFO
                }
            };
        if !on {
            return;
        }

        let count = match channel.count {
            0 if DS::I == 1 => 0x20_0000,
            0 if idx == 3 => 0x1_0000,
            0 => 0x4000,
            _ => channel.count.u32(),
        };

        let src_mod = Self::get_step(ctrl.src_addr());
        let dst_mod = match ctrl.dest_addr() {
            AddrControl::IncReload => {
                // Reload DST + Increment
                channel.dst = channel.dad
                    & if DS::I == 0 {
                        DST_MASK_7[idx.us()]
                    } else {
                        0xFFF_FFFF
                    };
                2
            }
            _ => Self::get_step(ctrl.dest_addr()),
        };

        if ctrl.is_32bit() {
            Self::perform_transfer::<DS, u32>(ds, channel, idx, count, src_mod * 2, dst_mod * 2);
        } else {
            Self::perform_transfer::<DS, u16>(ds, channel, idx, count, src_mod, dst_mod);
        }

        if !ctrl.repeat_en() || ctrl.timing() == Timing::Now {
            // Disable if reload is not enabled or it's an immediate transfer
            ds.dmas[DS::I].channels[idx].ctrl.set_dma_en(false);
        }
        if ctrl.irq_en() {
            // Fire interrupt if configured
            ds.cpu()
                .request_interrupt_with_index(Interrupt::Dma0 as u16 + idx.u16());
        }
    }

    /// Perform a transfer.
    fn perform_transfer<DS: NdsCpu, T: RwType>(
        ds: &mut DS,
        mut channel: Dma,
        idx: usize,
        count: u32,
        src_mod: i32,
        dst_mod: i32,
    ) {
        ds.tick(2);
        let mut kind = NONSEQ | DMA;

        // First, align SRC/DST
        let align = T::WIDTH - 1;
        channel.src &= !align;
        channel.dst &= !align;

        let cpu_outer = ds.cpu();
        let ds = &mut cpu_outer.bus;
        let cpu = &mut cpu_outer.state;
        for _ in 0..count {
            let value = ds.read::<T>(cpu, Address(channel.src), kind).u32();
            ds.write::<T>(cpu, Address(channel.dst), T::from_u32(value), kind);

            channel.src = channel.src.wrapping_add_signed(src_mod);
            channel.dst = channel.dst.wrapping_add_signed(dst_mod);
            // Only first is NonSeq
            kind = SEQ;
            ds.handle_events(cpu);
        }

        // Put last value into cache
        if T::WIDTH == 4 {
            ds.dmas[DS::I].cache =
                ds.get::<u32>(cpu, Address(channel.src.wrapping_add_signed(-src_mod)));
        } else {
            let value = ds.get::<u16>(cpu, Address(channel.src.wrapping_add_signed(-src_mod)));
            ds.dmas[DS::I].cache = word(value, value);
        }
    }

    /// Get the step with which to change SRC/DST registers after every write.
    /// Multiplied by 2 for word-sized DMAs.
    /// Inc+Reload handled separately.
    fn get_step(control: AddrControl) -> i32 {
        match control {
            AddrControl::Increment => 2,
            AddrControl::Decrement => -2,
            _ => 0,
        }
    }
}

/// Reason for why a DMA transfer attempt was initiated.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Reason {
    /// The control register was written.
    CtrlWrite,
    /// The PPU entered HBlank.
    HBlank,
    /// The PPU entered VBlank.
    VBlank,
    /// Main memory display (?)
    MemoryDisplay,
    /// Cartridge has data
    CartridgeReady,
}

#[bitfield]
#[repr(u16)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DmaControl {
    #[skip]
    __: B5,
    pub dest_addr: AddrControl,
    pub src_addr: AddrControl,
    pub repeat_en: bool,
    pub is_32bit: bool,
    pub timing_ext: bool,
    pub timing: Timing,
    pub irq_en: bool,
    pub dma_en: bool,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum AddrControl {
    Increment = 0,
    Decrement = 1,
    Fixed = 2,
    IncReload = 3,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
#[bits = 2]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Timing {
    Now = 0,
    VBlank = 1,
    HBlank = 2,
    Special = 3,
}
