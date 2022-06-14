use crate::{
    ggc::{
        io::{
            addr::{DMA, HDMA_DEST_HIGH, HDMA_DEST_LOW, HDMA_SRC_HIGH, HDMA_SRC_LOW, LCDC},
            ppu::DISP_EN,
            Mmu,
        },
        GameGirl,
    },
    numutil::NumExt,
};
use serde::{Deserialize, Serialize};

/// OAM DMA transfer available on DMG and CGB.
/// This implementation writes everything at once
/// once the timer of 648 cycles is up.
#[derive(Default, Deserialize, Serialize)]
pub struct Dma {
    time_left: i16,
}

impl Dma {
    pub fn step(gg: &mut GameGirl, m_cycles: u16) {
        if gg.mmu.dma.time_left <= 0 {
            return;
        }
        gg.mmu.dma.time_left -= m_cycles as i16;
        if gg.mmu.dma.time_left > 0 {
            return;
        }
        let mut src = gg.mmu[DMA].u16() * 0x100;
        for dest in 0xFE00..0xFEA0 {
            gg.mmu.write(dest, gg.mmu.read(src));
            src += 1;
        }
    }

    pub fn start(&mut self) {
        self.time_left = 162;
    }
}

/// HDMA VRAM transfer available only on CGB.
#[derive(Default, Deserialize, Serialize)]
pub struct Hdma {
    source: u16,
    dest: u16,

    pub transfer_left: i16,
    hblank_transferring: bool,
    gdma_queued: bool,
    pub ppu_in_hblank: bool,
}

impl Hdma {
    pub fn step(gg: &mut GameGirl) {
        if !gg.mmu.cgb {
            return;
        }
        if gg.mmu.hdma.gdma_queued {
            gg.mmu.hdma.gdma_queued = false;
            gg.advance_clock(1);
            while gg.mmu.hdma.transfer_left >= 0 {
                Self::advance_transfer(gg);
            }
        }

        if !Self::can_advance_hblank(gg) {
            return;
        }

        gg.advance_clock(1);
        Self::advance_transfer(gg);
        if gg.mmu.hdma.transfer_left < 0 {
            gg.mmu.hdma.hblank_transferring = false;
            gg.mmu.hdma.transfer_left = 0xFF;
        }
    }

    pub fn write_start(mmu: &mut Mmu, value: u8) {
        if mmu.hdma.hblank_transferring && !value.is_bit(7) {
            mmu.hdma.hblank_transferring = false;
            mmu.hdma.transfer_left |= 0x80;
        } else {
            mmu.hdma.source = (mmu[HDMA_SRC_LOW].u16() & 0xF0) | (mmu[HDMA_SRC_HIGH].u16() << 8);
            mmu.hdma.dest =
                (mmu[HDMA_DEST_LOW].u16() & 0xF0) | ((mmu[HDMA_DEST_HIGH].u16() & 0x1F) << 8);
            mmu.hdma.dest = (mmu.hdma.dest & 0x1FFF) | 0x8000;
            mmu.hdma.transfer_left = value as i16 & 0x7F;
            mmu.hdma.hblank_transferring = value.is_bit(7);
            mmu.hdma.gdma_queued = !mmu.hdma.hblank_transferring;
        }
    }

    fn can_advance_hblank(gg: &mut GameGirl) -> bool {
        let possible = gg.mmu.hdma.hblank_transferring
            && (gg.mmu.hdma.ppu_in_hblank || !gg.mmu[LCDC].is_bit(DISP_EN));
        gg.mmu.hdma.ppu_in_hblank = false;
        possible
    }

    fn advance_transfer(gg: &mut GameGirl) {
        for _ in 0..0x10 {
            gg.mmu
                .write(gg.mmu.hdma.dest, gg.mmu.read(gg.mmu.hdma.source));
            gg.mmu.hdma.source += 1;
            gg.mmu.hdma.dest += 1;
        }
        // 8 at once is 1 too much, split it
        gg.advance_clock(4);
        gg.advance_clock(4);
        gg.mmu.hdma.transfer_left -= 1;
    }
}
