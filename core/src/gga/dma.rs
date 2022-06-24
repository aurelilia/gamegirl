use serde::{Deserialize, Serialize};

use crate::{
    gga::{
        addr::DISPSTAT,
        cpu::{Cpu, Interrupt},
        graphics::{HBLANK, VBLANK},
        Access, GameGirlAdv,
    },
    numutil::{word, NumExt},
};

/// GGA's 4 DMA channels.
#[derive(Default, Deserialize, Serialize)]
pub struct Dmas {
    /// Internal source registers
    src: [u32; 4],
    /// Internal destination registers
    dst: [u32; 4],
}

impl Dmas {
    /// Update all DMAs to see if they need ticking.
    /// Called on V/Hblank.
    pub fn update(gg: &mut GameGirlAdv) {
        for idx in 0..4 {
            let base = Self::base_addr(idx);
            Self::step_dma::<false>(gg, idx, base, gg[base + 0xA]);
        }
    }

    /// Update a given DMA after it's control register was written.
    pub fn update_idx(gg: &mut GameGirlAdv, idx: u16, new_ctrl: u16) {
        let base = Self::base_addr(idx);
        let old_ctrl = gg[base + 0xA];
        if !old_ctrl.is_bit(15) && new_ctrl.is_bit(15) {
            // Reload SRC/DST
            let src = word(gg[base], gg[base + 2]);
            let dst = word(gg[base + 4], gg[base + 6]);
            gg.dma.src[idx.us()] = src;
            gg.dma.dst[idx.us()] = dst;
        }

        gg[base + 0xA] = new_ctrl;
        Self::step_dma::<false>(gg, idx, base, new_ctrl);
    }

    /// Try to perform a special transfer, if the DMA is otherwise configured
    /// for it.
    pub fn check_special_transfer(gg: &mut GameGirlAdv, idx: u16) {
        let base = Self::base_addr(idx);
        Self::step_dma::<true>(gg, idx, base, gg[base + 0xA]);
    }

    /// Get the destination register for a DMA; this is not the internal one.
    pub fn get_dest(gg: &mut GameGirlAdv, idx: u16) -> u32 {
        let base = Self::base_addr(idx);
        word(gg[base + 4], gg[base + 6])
    }

    /// Step a DMA and perform a transfer if possible.
    fn step_dma<const SPECIAL: bool>(gg: &mut GameGirlAdv, idx: u16, base: u32, ctrl: u16) {
        let on = ctrl.is_bit(15)
            && match ctrl.bits(12, 2) {
                0 => true,
                1 => gg[DISPSTAT].is_bit(VBLANK),
                2 => gg[DISPSTAT].is_bit(HBLANK),
                _ => SPECIAL,
            };
        if !on {
            return;
        }

        let count = gg[base + 8];
        let count = match count {
            _ if SPECIAL => 4,
            0 if idx == 3 => 0x1_0000,
            0 => 0x4000,
            _ => count.u32(),
        };
        let src_mod = Self::get_step(ctrl.bits(7, 2));
        let dst_mod = if SPECIAL {
            0
        } else {
            Self::get_step(ctrl.bits(5, 2))
        };
        if SPECIAL || ctrl.is_bit(10) {
            Self::perform_transfer::<true>(gg, idx.us(), count, src_mod * 2, dst_mod * 2);
        } else {
            Self::perform_transfer::<false>(gg, idx.us(), count, src_mod, dst_mod);
        }

        if !ctrl.is_bit(9) || ctrl.bits(12, 2) == 0 {
            // Disable if reload is not enabled
            gg[base + 0xA] = ctrl.set_bit(15, false)
        }
        if ctrl.is_bit(14) {
            // Fire interrupt if configured
            Cpu::request_interrupt_idx(gg, Interrupt::Dma0 as u16 + idx)
        }
    }

    /// Perform a transfer.
    fn perform_transfer<const WORD: bool>(
        gg: &mut GameGirlAdv,
        idx: usize,
        count: u32,
        src_mod: i32,
        dst_mod: i32,
    ) {
        let mut kind = Access::NonSeq;
        for _ in 0..count {
            if WORD {
                let value = gg.read_word(gg.dma.src[idx], kind);
                gg.write_word(gg.dma.dst[idx], value, kind);
            } else {
                let value = gg.read_hword(gg.dma.src[idx], kind).u16();
                gg.write_hword(gg.dma.dst[idx], value, kind);
            }

            gg.dma.src[idx] = gg.dma.src[idx].wrapping_add_signed(src_mod);
            gg.dma.dst[idx] = gg.dma.dst[idx].wrapping_add_signed(dst_mod);
            // Only first is NonSeq
            kind = Access::Seq;
        }
    }

    /// Get the step with which to change SRC/DST registers after every write.
    /// Multiplied by 2 for word-sized DMAs.
    fn get_step(bits: u16) -> i32 {
        match bits {
            0 => 2,
            1 => -2,
            2 => 0,
            _ => 0, // TODO
        }
    }

    /// Get the base address for a DMA (First register: SRC low)
    fn base_addr(idx: u16) -> u32 {
        0xB0 + (idx.u32() * 0xC)
    }
}
