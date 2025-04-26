// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use core::mem;

use armchair::{
    access::{DMA, NONSEQ, SEQ},
    interface::{Bus, RwType},
    Address, Interrupt, RelativeOffset,
};
use arrayvec::ArrayVec;
use common::{
    components::io::IoSection,
    numutil::{word, NumExt},
};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use crate::{cpu::GgaFullBus, hw::cartridge::SaveType};

const SRC_MASK: [u32; 4] = [0x7FF_FFFF, 0xFFF_FFFF, 0xFFF_FFFF, 0xFFF_FFFF];
const DST_MASK: [u32; 4] = [0x7FF_FFFF, 0x7FF_FFFF, 0x7FF_FFFF, 0xFFF_FFFF];

#[derive(Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Dma {
    pub sad: u32,
    pub dad: u32,
    pub count: u16,
    pub ctrl: DmaControl,

    /// Internal source register
    src: Address,
    /// Internal destination register
    dst: Address,
}

/// GGA's 4 DMA channels.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Dmas {
    pub channels: [Dma; 4],
    /// Internal cache shared between DMAs
    pub(crate) cache: u32,
    /// Currently running DMA, or 99
    pub(crate) running: u16,
    /// DMAs waiting to run after current.
    queued: ArrayVec<(u16, Reason), 3>,
    /// PC when the last DMA finished (for lingering bus behavior)
    pub(crate) pc_at_last_end: Address,
}

impl Dmas {
    /// Update all DMAs to see if they need ticking.
    pub fn update_all(gg: &mut GgaFullBus<'_>, reason: Reason) {
        for idx in 0..4 {
            Self::step_dma(gg, idx, reason);
        }
    }

    /// Update a given DMA after it's control register was written.
    pub fn ctrl_write(gg: &mut GgaFullBus<'_>, idx: usize, new_ctrl: IoSection<u16>) {
        let channel = &mut gg.dma.channels[idx];
        let old_ctrl = channel.ctrl;
        let mut new_ctrl = new_ctrl.apply_io_ret(&mut channel.ctrl);

        if !old_ctrl.dma_en() && new_ctrl.dma_en() {
            // Reload SRC/DST
            channel.src = Address(channel.sad & SRC_MASK[idx]);
            channel.dst = Address(channel.dad & DST_MASK[idx]);
        }

        new_ctrl.set_dma3_gamepak_drq_en(new_ctrl.dma3_gamepak_drq_en() && idx == 3);
        gg.dma.channels[idx].ctrl = new_ctrl;
        Self::step_dma(gg, idx, Reason::CtrlWrite);
    }

    /// Try to perform a FIFO transfer, if the DMA is otherwise configured for
    /// it.
    pub fn try_fifo_transfer(gg: &mut GgaFullBus<'_>, idx: usize) {
        Self::step_dma(gg, idx, Reason::Fifo);
    }

    /// Step a DMA and perform a transfer if possible.
    fn step_dma(gg: &mut GgaFullBus<'_>, idx: usize, reason: Reason) {
        let mut channel = gg.dma.channels[idx];
        let ctrl = channel.ctrl;

        let is_fifo = reason == Reason::Fifo;
        let is_vid_capture = idx == 3
            && (2..162).contains(&gg.ppu.regs.vcount)
            && reason == Reason::HBlank
            && ctrl.timing() == Timing::Special;
        let is_on = ctrl.dma_en()
            && match ctrl.timing() {
                Timing::Now => reason == Reason::CtrlWrite,
                Timing::VBlank => reason == Reason::VBlank,
                Timing::HBlank => reason == Reason::HBlank && gg.ppu.regs.vcount < 160,
                Timing::Special => is_fifo || is_vid_capture,
            };
        if !is_on {
            return;
        }
        if gg.dma.running <= idx.u16() {
            gg.dma.queued.try_push((idx.u16(), reason)).ok();
            return;
        }

        let prev_dma = mem::replace(&mut gg.dma.running, idx.u16());

        let count = match channel.count {
            _ if is_fifo => 4,
            0 if idx == 3 => 0x1_0000,
            0 => 0x4000,
            _ => channel.count.u32(),
        };

        let src_mod = match channel.src.0 {
            0x800_0000..=0xDFF_FFFF => RelativeOffset::HW,
            _ => Self::get_step(ctrl.src_addr()),
        };

        let dst_mod = match ctrl.dest_addr() {
            _ if is_fifo => RelativeOffset(0),
            AddrControl::IncReload => {
                // Reload DST + Increment
                channel.dst = Address(channel.dad & DST_MASK[idx]);
                RelativeOffset::HW
            }
            _ => Self::get_step(ctrl.dest_addr()),
        };

        if is_fifo || ctrl.is_32bit() {
            Self::perform_transfer::<u32>(gg, channel, idx, count, src_mod.mul(2), dst_mod.mul(2));
        } else if idx == 3 {
            // Maybe alert EEPROM, if the cart has one
            if let SaveType::Eeprom(eeprom) = &mut gg.cart.save_type {
                eeprom.dma3_started(channel.dst, count);
            }
            Self::perform_transfer::<u16>(gg, channel, 3, count, src_mod, dst_mod);
        } else {
            Self::perform_transfer::<u16>(gg, channel, idx, count, src_mod, dst_mod);
        }

        if !ctrl.repeat_en()
            || ctrl.timing() == Timing::Now
            || (is_vid_capture && gg.ppu.regs.vcount == 161)
        {
            // Disable if reload is not enabled, it's an immediate transfer, or end of video
            // capture
            gg.dma.channels[idx].ctrl.set_dma_en(false);
        }
        if ctrl.irq_en() {
            // Fire interrupt if configured
            gg.cpu
                .request_interrupt_with_index(gg.bus, Interrupt::Dma0 as u16 + idx.u16());
        }

        gg.dma.running = prev_dma;
        if let Some((dma, reason)) = gg.dma.queued.pop() {
            Self::step_dma(gg, dma.us(), reason);
        }
    }

    /// Perform a transfer.
    fn perform_transfer<T: RwType>(
        gg: &mut GgaFullBus<'_>,
        mut channel: Dma,
        idx: usize,
        count: u32,
        src_mod: RelativeOffset,
        dst_mod: RelativeOffset,
    ) {
        gg.tick(2);
        if channel.dst.0 < 0x200_0000 {
            return;
        }

        let mut kind = NONSEQ | DMA;
        if channel.src.0 >= 0x200_0000 {
            channel.src = channel.src.align(T::WIDTH);
            channel.dst = channel.dst.align(T::WIDTH);

            for _ in 0..count {
                let value = gg.bus.read::<T>(gg.cpu, channel.src, kind).u32();
                gg.bus
                    .write::<T>(gg.cpu, channel.dst, T::from_u32(value), kind);

                channel.src = channel.src.add_rel(src_mod);
                channel.dst = channel.dst.add_rel(dst_mod);
                // Only first is NonSeq
                kind = SEQ | DMA;
                gg.advance_clock();
            }

            // Put last value into cache
            if T::WIDTH == 4 {
                gg.dma.cache = gg.get::<u32>(channel.src.add_rel(-src_mod));
            } else {
                let value = gg.get::<u16>(channel.src.add_rel(-src_mod));
                gg.dma.cache = word(value, value);
            }
        } else {
            for _ in 0..count {
                if T::WIDTH == 4 {
                    gg.bus.write::<u32>(gg.cpu, channel.dst, gg.dma.cache, kind);
                } else if channel.dst.0.is_bit(1) {
                    gg.bus
                        .write::<u16>(gg.cpu, channel.dst, (gg.dma.cache >> 16).u16(), kind);
                } else {
                    gg.bus
                        .write::<u16>(gg.cpu, channel.dst, gg.dma.cache.u16(), kind);
                }
                channel.src = channel.src.add_rel(src_mod);
                channel.dst = channel.dst.add_rel(dst_mod);
                // Only first is NonSeq
                kind = SEQ | DMA;
                gg.advance_clock();
            }
        }
        gg.dma.pc_at_last_end = gg.cpu.pc();
        gg.dma.channels[idx] = channel;
    }

    /// Get the step with which to change SRC/DST registers after every write.
    /// Multiplied by 2 for word-sized DMAs.
    /// Inc+Reload handled separately.
    fn get_step(control: AddrControl) -> RelativeOffset {
        match control {
            AddrControl::Increment => RelativeOffset::HW,
            AddrControl::Decrement => -RelativeOffset::HW,
            _ => RelativeOffset(0),
        }
    }
}

impl Default for Dmas {
    fn default() -> Self {
        Self {
            channels: [Dma::default(); 4],
            running: 99,
            queued: ArrayVec::new(),
            cache: 0,
            pc_at_last_end: Address(0),
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
    /// A FIFO sound channel is requesting new samples.
    Fifo,
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
    pub dma3_gamepak_drq_en: bool,
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
