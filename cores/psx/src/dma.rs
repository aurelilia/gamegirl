// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

#![allow(clippy::identity_op)]

use common::numutil::NumExt;
use modular_bitfield::{
    bitfield,
    specifiers::{B2, B3, B4, B5, B6},
    BitfieldSpecifier,
};

use crate::{
    addr::{DMAADDR, DMABASE, DMABLOCKCTRL, DMACHCTRL, PORT_GPU, PORT_OTC},
    gpu::Gpu,
    PlayStation,
};

#[bitfield]
#[repr(u32)]
#[derive(Debug)]
pub struct DmaChControl {
    is_from_ram: bool,
    step_backward: bool,
    unused: B6,
    chop_enable: bool,
    sync_mode: SyncMode,
    unused2: B5,
    chop_dma_window: B3,
    unused3: bool,
    chop_cpu_window: B3,
    unused4: bool,
    enable: bool,
    unused5: B3,
    trigger: bool,
    pause: bool,
    unused6: B2,
}

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug)]
pub enum SyncMode {
    Manual = 0,
    Block = 1,
    LinkedList = 2,
    Reserved = 3,
}

pub struct Dma {}

impl Dma {
    pub fn maybe_trigger(ps: &mut PlayStation, addr: u32) {
        let dma = ((addr - DMABASE) & 0x70) >> 4;
        let ctrl = Self::ctrl(ps, dma);

        let triggered = match ctrl.sync_mode() {
            SyncMode::Manual => ctrl.trigger(),
            SyncMode::Block | SyncMode::LinkedList => true, // ?

            SyncMode::Reserved => {
                log::warn!("Reserved DMA transfer configured?");
                false
            }
        };
        log::debug!("DMA: {triggered} on DMA {dma}");
        if ctrl.enable() && triggered {
            Self::perform_transfer(ps, dma, ctrl);
        }
    }

    fn perform_transfer(ps: &mut PlayStation, dma: u32, ctrl: DmaChControl) {
        let bctrl = ps[Self::addr(dma, DMABLOCKCTRL)];
        match ctrl.sync_mode() {
            SyncMode::Manual => Self::regular_transfer(ps, dma, ctrl, bctrl & 0xFFFF),
            SyncMode::Block => {
                let block_size = bctrl & 0xFFFF;
                let block_cnt = bctrl >> 16;
                Self::regular_transfer(ps, dma, ctrl, block_cnt * block_size);
            }
            SyncMode::LinkedList => Self::ll_transfer(ps, dma, ctrl),
            SyncMode::Reserved => log::warn!("Reserved DMA transfer requested?"),
        }
    }

    fn regular_transfer(ps: &mut PlayStation, dma: u32, ctrl: DmaChControl, size: u32) {
        let mut addr = ps[Self::addr(dma, DMAADDR)];
        let increment = if ctrl.step_backward() { -4 } else { 4 };
        log::debug!("DMA{dma} transfer: Size: {size}, Address {addr:08X}, Control: {ctrl:#?}");

        let mut remaining = size;
        while remaining > 0 {
            let current = addr & 0x1F_FFFC;
            if ctrl.is_from_ram() {
                let src = ps.get::<u32>(current);
                match dma {
                    PORT_GPU => Gpu::gp0_write(ps, src),
                    port => {
                        log::debug!("Sending 0x{src:08X} via DMA to Port {port}: unimplemented")
                    }
                }
            } else {
                let src = match dma {
                    PORT_OTC if remaining == 1 => 0xFF_FFFF,
                    PORT_OTC => addr.wrapping_sub(4) & 0x1F_FFFC,

                    _ => panic!("Unknown DMA port"),
                };
                ps.set(current, src);
            }

            addr = addr.wrapping_add_signed(increment);
            remaining -= 1;
        }

        Self::transfer_finish(ps, dma, ctrl)
    }

    fn ll_transfer(ps: &mut PlayStation, dma: u32, ctrl: DmaChControl) {
        let mut addr = ps[Self::addr(dma, DMAADDR)];
        assert!(dma == PORT_GPU, "LL not support for non-GPU DMA!");
        assert!(ctrl.is_from_ram(), "LL DMA must be from RAM!");
        log::debug!("DMA{dma} LL transfer: Address {addr:08X}, Control: {ctrl:#?}");

        loop {
            let header = ps.get::<u32>(addr);
            let mut remaining = header >> 24;
            while remaining > 0 {
                addr = addr.wrapping_add(4) & 0x1F_FFFC;
                let command = ps.get(addr);
                Gpu::gp0_write(ps, command);
                remaining -= 1;
            }

            if header.is_bit(23) {
                break;
            }

            addr = header & 0x1F_FFFC;
        }

        Self::transfer_finish(ps, dma, ctrl)
    }

    fn transfer_finish(ps: &mut PlayStation, dma: u32, mut ctrl: DmaChControl) {
        ctrl.set_enable(false);
        ctrl.set_trigger(false);
        ps[Self::addr(dma, DMACHCTRL)] = ctrl.into();
        log::debug!("DMA{dma} finished.")
    }

    fn ctrl(ps: &PlayStation, dma: u32) -> DmaChControl {
        DmaChControl::from(ps[Self::addr(dma, DMACHCTRL)])
    }

    fn addr(dma: u32, offs: u32) -> u32 {
        DMABASE + (dma << 4) + offs
    }
}
