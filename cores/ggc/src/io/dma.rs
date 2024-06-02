// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{numutil::NumExt, TimeS};

use super::addr::HDMA_START;
use crate::{
    io::{
        addr::{HDMA_DEST_HIGH, HDMA_DEST_LOW, HDMA_SRC_HIGH, HDMA_SRC_LOW, LCDC},
        ppu::DISP_EN,
        scheduling::GGEvent,
    },
    GameGirl,
};

pub fn dma_written(gg: &mut GameGirl, value: u8) {
    gg.dma = value;
    let time = 648 / gg.speed as TimeS;
    gg.mem.dma_restarted = gg.scheduler.cancel_single(GGEvent::DMAFinish);
    gg.scheduler.schedule(GGEvent::DMAFinish, time);
    gg.mem.pending_dma = Some(gg.scheduler.now());
}

/// OAM DMA transfer available on DMG and CGB.
/// This implementation writes everything at once
/// once the timer of 648 cycles is up.
pub fn do_oam_dma(gg: &mut GameGirl) {
    let src = gg.dma.u16() * 0x100;
    let mut src = if src > 0xDF00 { src - 0x2000 } else { src };
    for dest in 0..0xA0 {
        gg.mem.oam[dest] = gg.get(src);
        src += 1;
    }
    gg.mem.pending_dma = None;
    gg.mem.dma_restarted = false;
}

/// HDMA VRAM transfer available only on CGB.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Hdma {
    source: u16,
    dest: u16,

    pub transfer_left: i16,
    pub hblank_transferring: bool,
}

impl Hdma {
    pub fn get(&self, addr: u16) -> u8 {
        match addr {
            HDMA_START => self.transfer_left as u8,
            _ => 0xFF,
        }
    }

    pub fn set(gg: &mut GameGirl, addr: u16, value: u8) {
        match addr {
            HDMA_START => Hdma::write_start(gg, value),
            HDMA_SRC_LOW => gg.hdma.source = (gg.hdma.source & 0xFF00) | (value.u16() & 0xF0),
            HDMA_SRC_HIGH => gg.hdma.source = (gg.hdma.source & 0x00FF) | (value.u16() << 8),
            HDMA_DEST_LOW => gg.hdma.dest = (gg.hdma.dest & 0xFF00) | (value.u16() & 0xF0),
            HDMA_DEST_HIGH => gg.hdma.dest = (gg.hdma.dest & 0x00FF) | ((value.u16() & 0x1F) << 8),
            _ => (),
        }
        gg.hdma.dest = (gg.hdma.dest & 0x1FFF) | 0x8000;
    }

    pub fn write_start(gg: &mut GameGirl, value: u8) {
        if gg.hdma.hblank_transferring && !value.is_bit(7) {
            gg.hdma.hblank_transferring = false;
            gg.hdma.transfer_left |= 0x80;
            gg.scheduler.cancel_single(GGEvent::HdmaTransferStep);
        } else {
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
        gg.advance_clock(1);
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
        gg.advance_clock(1);
        while gg.hdma.transfer_left >= 0 {
            Self::advance_transfer(gg);
        }
    }

    fn advance_transfer(gg: &mut GameGirl) {
        for _ in 0..0x10 {
            let src: u8 = gg.get(gg.hdma.source);
            gg.set(gg.hdma.dest, src);
            gg.hdma.source = gg.hdma.source.wrapping_add(1);
            gg.hdma.dest = gg.hdma.dest.wrapping_add(1);
        }
        // 8 at once is 1 too much, split it
        gg.advance_clock(2);
        gg.advance_clock(2);
        gg.advance_clock(2);
        gg.advance_clock(2);
        gg.hdma.transfer_left -= 1;
    }
}
