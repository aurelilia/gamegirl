use crate::{
    gga::{
        addr::DISPSTAT,
        cpu::{Cpu, Interrupt},
        graphics::{HBLANK, VBLANK},
        Access, GameGirlAdv,
    },
    numutil::{word, NumExt},
};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct Dmas {}

impl Dmas {
    pub fn update(gg: &mut GameGirlAdv) {
        for idx in 0..4 {
            let base = Self::base_addr(idx);
            Self::step_dma(gg, idx, base, gg[base + 0xA]);
        }
    }

    pub fn update_idx(gg: &mut GameGirlAdv, idx: u16, ctrl: u16) {
        let base = Self::base_addr(idx);
        Self::step_dma(gg, idx, base, ctrl);
    }

    fn step_dma(gg: &mut GameGirlAdv, idx: u16, base: u32, ctrl: u16) {
        let on = ctrl.is_bit(15)
            && match ctrl.bits(12, 2) {
                0 => true,
                1 => gg[DISPSTAT].is_bit(VBLANK),
                2 => gg[DISPSTAT].is_bit(HBLANK),
                _ => false, // TODO sound fifo/video capture
            };
        if !on {
            return;
        }

        // TODO actually store SRC/DST/WORD in a separate internal register,
        // they are not reread like this if repeat is enabled
        let src = word(gg[base], gg[base + 2]);
        let dst = word(gg[base + 4], gg[base + 6]);
        let count = gg[base + 8];
        let count = match count {
            0 if idx == 3 => 0x1_0000,
            0 => 0x4000,
            _ => count.u32(),
        };
        let src_mod = Self::get_step(ctrl.bits(7, 2));
        let dst_mod = Self::get_step(ctrl.bits(5, 2));
        if ctrl.is_bit(10) {
            Self::perform_transfer::<true>(gg, src, dst, count, src_mod * 2, dst_mod * 2);
        } else {
            Self::perform_transfer::<false>(gg, src, dst, count, src_mod, dst_mod);
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

    fn perform_transfer<const WORD: bool>(
        gg: &mut GameGirlAdv,
        mut src: u32,
        mut dst: u32,
        count: u32,
        src_mod: i32,
        dst_mod: i32,
    ) {
        let mut kind = Access::NonSeq;
        for _ in 0..count {
            if WORD {
                let value = gg.read_word(src, kind);
                gg.write_word(dst, value, kind);
            } else {
                let value = gg.read_hword(src, kind).u16();
                gg.write_hword(dst, value, kind);
            }

            src = src.wrapping_add_signed(src_mod);
            dst = dst.wrapping_add_signed(dst_mod);
            // Only first is NonSeq
            kind = Access::Seq;
        }
    }

    fn get_step(bits: u16) -> i32 {
        match bits {
            0 => 2,
            1 => -2,
            2 => 0,
            _ => 0, // TODO
        }
    }

    fn base_addr(idx: u16) -> u32 {
        0xB0 + (idx.u32() * 0xC)
    }
}
