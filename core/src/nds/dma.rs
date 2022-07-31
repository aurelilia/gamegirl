// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

use crate::{
    components::arm::{interface::RwType, Access, Cpu, Interrupt},
    nds::{addr::VCOUNT, NdsCpu},
    numutil::{word, NumExt},
};

const SRC_MASK_7: [u32; 4] = [0x7FF_FFFF, 0xFFF_FFFF, 0xFFF_FFFF, 0xFFF_FFFF];
const DST_MASK_7: [u32; 4] = [0x7FF_FFFF, 0x7FF_FFFF, 0x7FF_FFFF, 0xFFF_FFFF];

/// NDS's 2x4 DMA channels.
/// This is separated by CPU.
#[derive(Default, Deserialize, Serialize)]
pub struct Dmas {
    /// Internal source registers
    src: [u32; 4],
    /// Internal destination registers
    dst: [u32; 4],
    /// Internal cache shared between DMAs
    pub(super) cache: u32,
}

impl Dmas {
    /// Update all DMAs to see if they need ticking.
    pub fn update_all(ds: &mut impl NdsCpu, reason: Reason) {
        for idx in 0..4 {
            let base = Self::base_addr(idx);
            Self::step_dma(ds, idx, base, ds[base + 0xA], reason);
        }
    }

    /// Update a given DMA after it's control register was written.
    pub fn ctrl_write<DS: NdsCpu>(ds: &mut DS, idx: u16, new_ctrl: u16) {
        let base = Self::base_addr(idx);
        let old_ctrl = ds[base + 0xA];
        if !old_ctrl.is_bit(15) && new_ctrl.is_bit(15) {
            // Reload SRC/DST
            let src = word(ds[base], ds[base + 2]);
            let dst = word(ds[base + 4], ds[base + 6]);
            if DS::I == 0 {
                // NDS7
                ds.dmas[DS::I].src[idx.us()] = src & SRC_MASK_7[idx.us()];
                ds.dmas[DS::I].dst[idx.us()] = dst & DST_MASK_7[idx.us()];
            } else {
                // NDS9
                ds.dmas[DS::I].src[idx.us()] = src & 0xFFF_FFFF;
                ds.dmas[DS::I].dst[idx.us()] = dst & 0xFFF_FFFF;
            }
        }

        ds[base + 0xA] = new_ctrl & if idx == 3 { 0xFFE0 } else { 0xF7E0 };
        Self::step_dma(ds, idx, base, new_ctrl, Reason::CtrlWrite);
    }

    /// Get the destination register for a DMA; this is not the internal one.
    pub fn get_dest(ds: &mut impl NdsCpu, idx: u16) -> u32 {
        let base = Self::base_addr(idx);
        word(ds[base + 4], ds[base + 6])
    }

    /// Step a DMA and perform a transfer if possible.
    fn step_dma<DS: NdsCpu>(ds: &mut DS, idx: u16, base: u32, ctrl: u16, reason: Reason) {
        let on = ctrl.is_bit(15)
            && if DS::I == 0 {
                // NDS7
                match ctrl.bits(12, 2) {
                    0 => reason == Reason::CtrlWrite,
                    1 => reason == Reason::VBlank,
                    2 => reason == Reason::CartridgeReady,
                    _ => false, // TODO wireless?
                }
            } else {
                // NDS9
                match ctrl.bits(11, 3) {
                    0 => reason == Reason::CtrlWrite,
                    1 => reason == Reason::VBlank,
                    2 => reason == Reason::HBlank && ds[VCOUNT] < 160,
                    3 => reason == Reason::HBlank && ds[VCOUNT] == 0,
                    4 => false, // TODO
                    5 => reason == Reason::CartridgeReady,
                    6 => false,
                    _ => false, // TODO
                }
            };
        if !on {
            return;
        }

        let count = ds[base + 8];
        let count = match count {
            0 if DS::I == 1 => 0x20_0000,
            0 if idx == 3 => 0x1_0000,
            0 => 0x4000,
            _ => count.u32(),
        };

        let src_mod = Self::get_step(ctrl.bits(7, 2));
        let dst_raw = ctrl.bits(5, 2);
        let dst_mod = match dst_raw {
            3 => {
                // Reload DST + Increment
                let dst = word(ds[base + 4], ds[base + 6]);
                ds.dmas[DS::I].dst[idx.us()] = dst
                    & if DS::I == 0 {
                        DST_MASK_7[idx.us()]
                    } else {
                        0xFFF_FFFF
                    };
                2
            }
            _ => Self::get_step(dst_raw),
        };

        let word_transfer = ctrl.is_bit(10);
        if word_transfer {
            Self::perform_transfer::<DS, u32>(ds, idx.us(), count, src_mod * 2, dst_mod * 2);
        } else {
            Self::perform_transfer::<DS, u16>(ds, idx.us(), count, src_mod, dst_mod);
        }

        if !ctrl.is_bit(9) || ctrl.bits(12, 2) == 0 {
            // Disable if reload is not enabled or it's an immediate transfer
            ds[base + 0xA] = ctrl.set_bit(15, false);
        }
        if ctrl.is_bit(14) {
            // Fire interrupt if configured
            Cpu::request_interrupt_idx(ds, Interrupt::Dma0 as u16 + idx);
        }
    }

    /// Perform a transfer.
    fn perform_transfer<DS: NdsCpu, T: RwType>(
        ds: &mut DS,
        idx: usize,
        count: u32,
        src_mod: i32,
        dst_mod: i32,
    ) {
        if ds.dmas[DS::I].dst[idx] < 0x200_0000 {
            return;
        }

        let mut kind = Access::NonSeq;
        if ds.dmas[DS::I].src[idx] >= 0x200_0000 {
            // First, align SRC/DST
            let align = T::WIDTH - 1;
            ds.dmas[DS::I].src[idx] &= !align;
            ds.dmas[DS::I].dst[idx] &= !align;

            for _ in 0..count {
                let value = ds.read::<T>(ds.dmas[DS::I].src[idx], kind).u32();
                ds.write::<T>(ds.dmas[DS::I].dst[idx], T::from_u32(value), kind);

                ds.dmas[DS::I].src[idx] = ds.dmas[DS::I].src[idx].wrapping_add_signed(src_mod);
                ds.dmas[DS::I].dst[idx] = ds.dmas[DS::I].dst[idx].wrapping_add_signed(dst_mod);
                // Only first is NonSeq
                kind = Access::Seq;
                ds.advance_clock();
            }

            // Put last value into cache
            if T::WIDTH == 4 {
                ds.dmas[DS::I].cache =
                    ds.get::<u32>(ds.dmas[DS::I].src[idx].wrapping_add_signed(-src_mod));
            } else {
                let value = ds.get::<u16>(ds.dmas[DS::I].src[idx].wrapping_add_signed(-src_mod));
                ds.dmas[DS::I].cache = word(value, value);
            }
        } else {
            for _ in 0..count {
                if T::WIDTH == 4 {
                    ds.write::<u32>(ds.dmas[DS::I].dst[idx], ds.dmas[DS::I].cache, kind);
                } else if ds.dmas[DS::I].dst[idx].is_bit(1) {
                    ds.write::<u16>(
                        ds.dmas[DS::I].dst[idx],
                        (ds.dmas[DS::I].cache >> 16).u16(),
                        kind,
                    );
                } else {
                    ds.write::<u16>(ds.dmas[DS::I].dst[idx], ds.dmas[DS::I].cache.u16(), kind);
                }
                ds.dmas[DS::I].src[idx] = ds.dmas[DS::I].src[idx].wrapping_add_signed(src_mod);
                ds.dmas[DS::I].dst[idx] = ds.dmas[DS::I].dst[idx].wrapping_add_signed(dst_mod);
                // Only first is NonSeq
                kind = Access::Seq;
                ds.advance_clock();
            }
        }
        ds.add_i_cycles(2);
    }

    /// Get the step with which to change SRC/DST registers after every write.
    /// Multiplied by 2 for word-sized DMAs.
    /// Inc+Reload handled separately.
    fn get_step(bits: u16) -> i32 {
        match bits {
            0 => 2,
            1 => -2,
            _ => 0,
        }
    }

    /// Get the base address for a DMA (First register: SRC low)
    fn base_addr(idx: u16) -> u32 {
        0xB0 + (idx.u32() * 0xC)
    }
}

/// Reason for why a DMA transfer attempt was initiated.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
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
