use serde::{Deserialize, Serialize};

use crate::{
    ggc::{
        io::{
            addr::{DMA, HDMA_DEST_HIGH, HDMA_DEST_LOW, HDMA_SRC_HIGH, HDMA_SRC_LOW, LCDC},
            ppu::DISP_EN,
            scheduling::GGEvent,
            Mmu,
        },
        GameGirl,
    },
    numutil::NumExt,
};

/// OAM DMA transfer available on DMG and CGB.
/// This implementation writes everything at once
/// once the timer of 648 cycles is up.
pub fn do_oam_dma(gg: &mut GameGirl) {
    let mut src = gg.mmu[DMA].u16() * 0x100;
    for dest in 0..0xA0 {
        gg.mmu.oam[dest] = gg.mmu.read(src);
        src += 1;
    }
}

/// HDMA VRAM transfer available only on CGB.
#[derive(Default, Deserialize, Serialize)]
pub struct Hdma {
    source: u16,
    dest: u16,

    pub transfer_left: i16,
    pub hblank_transferring: bool,
}

impl Hdma {
    pub fn write_start(mmu: &mut Mmu, value: u8) {
        if mmu.hdma.hblank_transferring && !value.is_bit(7) {
            mmu.hdma.hblank_transferring = false;
            mmu.hdma.transfer_left |= 0x80;
            mmu.scheduler.cancel(GGEvent::HdmaTransferStep);
        } else {
            mmu.hdma.source = (mmu[HDMA_SRC_LOW].u16() & 0xF0) | (mmu[HDMA_SRC_HIGH].u16() << 8);
            mmu.hdma.dest =
                (mmu[HDMA_DEST_LOW].u16() & 0xF0) | ((mmu[HDMA_DEST_HIGH].u16() & 0x1F) << 8);
            mmu.hdma.dest = (mmu.hdma.dest & 0x1FFF) | 0x8000;
            mmu.hdma.transfer_left = value as i16 & 0x7F;
            mmu.hdma.hblank_transferring = value.is_bit(7);

            if !mmu.hdma.hblank_transferring {
                mmu.scheduler.schedule(GGEvent::GdmaTransfer, 2);
            } else if !mmu[LCDC].is_bit(DISP_EN) {
                // Only reschedule when PPU is off, when it is not, PPU will schedule next
                // transfer step.
                mmu.scheduler.schedule(GGEvent::HdmaTransferStep, 2);
            }
        }
    }

    pub fn handle_hdma(gg: &mut GameGirl) {
        Self::advance_transfer(gg);
        if gg.mmu.hdma.transfer_left < 0 {
            gg.mmu.hdma.hblank_transferring = false;
            gg.mmu.hdma.transfer_left = 0xFF;
        } else if !gg.mmu[LCDC].is_bit(DISP_EN) {
            // Only reschedule when PPU is off, when it is not, PPU will schedule next
            // transfer step.
            gg.mmu.scheduler.schedule(GGEvent::HdmaTransferStep, 2);
        }
    }

    pub fn handle_gdma(gg: &mut GameGirl) {
        while gg.mmu.hdma.transfer_left >= 0 {
            Self::advance_transfer(gg);
        }
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
