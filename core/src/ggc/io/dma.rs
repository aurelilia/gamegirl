use serde::{Deserialize, Serialize};

use crate::{
    ggc::{
        io::{
            addr::{DMA, HDMA_DEST_HIGH, HDMA_DEST_LOW, HDMA_SRC_HIGH, HDMA_SRC_LOW, LCDC},
            ppu::DISP_EN,
            scheduling::GGEvent,
        },
        GameGirl,
    },
    numutil::NumExt,
};

/// OAM DMA transfer available on DMG and CGB.
/// This implementation writes everything at once
/// once the timer of 648 cycles is up.
pub fn do_oam_dma(gg: &mut GameGirl) {
    let src = gg[DMA].u16() * 0x100;
    let mut src = if src > 0xDF00 { src - 0x2000 } else { src };
    for dest in 0..0xA0 {
        gg.mem.oam[dest] = gg.get8(src);
        src += 1;
    }
    gg.mem.dma_active = false;
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
    pub fn write_start(gg: &mut GameGirl, value: u8) {
        if gg.hdma.hblank_transferring && !value.is_bit(7) {
            gg.hdma.hblank_transferring = false;
            gg.hdma.transfer_left |= 0x80;
            gg.scheduler.cancel(GGEvent::HdmaTransferStep);
        } else {
            gg.hdma.source = (gg[HDMA_SRC_LOW].u16() & 0xF0) | (gg[HDMA_SRC_HIGH].u16() << 8);
            gg.hdma.dest =
                (gg[HDMA_DEST_LOW].u16() & 0xF0) | ((gg[HDMA_DEST_HIGH].u16() & 0x1F) << 8);
            gg.hdma.dest = (gg.hdma.dest & 0x1FFF) | 0x8000;
            gg.hdma.transfer_left = value as i16 & 0x7F;
            gg.hdma.hblank_transferring = value.is_bit(7);

            if !gg.hdma.hblank_transferring {
                gg.scheduler.schedule(GGEvent::GdmaTransfer, 2);
            } else if !gg[LCDC].is_bit(DISP_EN) {
                // Only reschedule when PPU is off, when it is not, PPU will schedule next
                // transfer step.
                gg.scheduler.schedule(GGEvent::HdmaTransferStep, 2);
            }
        }
    }

    pub fn handle_hdma(gg: &mut GameGirl) {
        Self::advance_transfer(gg);
        if gg.hdma.transfer_left < 0 {
            gg.hdma.hblank_transferring = false;
            gg.hdma.transfer_left = 0xFF;
        } else if !gg[LCDC].is_bit(DISP_EN) {
            // Only reschedule when PPU is off, when it is not, PPU will schedule next
            // transfer step.
            gg.scheduler.schedule(GGEvent::HdmaTransferStep, 2);
        }
    }

    pub fn handle_gdma(gg: &mut GameGirl) {
        while gg.hdma.transfer_left >= 0 {
            Self::advance_transfer(gg);
        }
    }

    fn advance_transfer(gg: &mut GameGirl) {
        for _ in 0..0x10 {
            gg.set8(gg.hdma.dest, gg.get8(gg.hdma.source));
            gg.hdma.source += 1;
            gg.hdma.dest += 1;
        }
        // 8 at once is 1 too much, split it
        gg.advance_clock(4);
        gg.advance_clock(4);
        gg.hdma.transfer_left -= 1;
    }
}
