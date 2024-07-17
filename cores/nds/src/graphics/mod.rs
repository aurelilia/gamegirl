// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::sync::Arc;

use arm_cpu::{Cpu, Interrupt};
use common::numutil::NumExt;
use ppu::{registers::DisplayStatus, HEIGHT, VBLANK_END};

use crate::{
    dma::{Dmas, Reason},
    scheduling::{NdsEvent, PpuEvent},
    Nds9,
};

mod ppu;

const KB: usize = 1024;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Vram {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub a: [u8; 128 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub b: [u8; 128 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub c: [u8; 128 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub d: [u8; 128 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub e: [u8; 64 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub g: [u8; 16 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub h: [u8; 32 * KB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub i: [u8; 16 * KB],
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Gpu {
    pub vram: Arc<Vram>,
    pub ppus: [ppu::Ppu; 2],

    pub(super) dispstat: DisplayStatus,
    pub(super) vcount: u16,
}

impl Gpu {
    pub fn handle_event(ds: &mut Nds9, event: PpuEvent, late_by: i64) {
        let (next_event, cycles) = match event {
            PpuEvent::HblankStart => {
                if ds.c.video_buffer.should_render_this_frame() {
                    ds.gpu.render_line();
                }

                Self::maybe_interrupt(ds, Interrupt::HBlank);
                Dmas::update_all(ds, Reason::HBlank);
                (PpuEvent::SetHblank, 46)
            }

            PpuEvent::SetHblank => {
                ds.gpu.dispstat.set_in_hblank(true);
                (PpuEvent::HblankEnd, 226)
            }

            PpuEvent::HblankEnd => {
                ds.gpu.vcount += 1;

                let vcount = ds.gpu.vcount;
                match () {
                    _ if ds.gpu.vcount == HEIGHT.u16() => {
                        ds.gpu.dispstat.set_in_vblank(true);
                        Self::maybe_interrupt(ds, Interrupt::VBlank);
                        Dmas::update_all(ds, Reason::VBlank);
                    }
                    // VBlank flag gets set one scanline early
                    _ if vcount == (VBLANK_END - 1) => {
                        ds.gpu.dispstat.set_in_vblank(false);
                    }
                    _ if vcount == VBLANK_END => {
                        ds.gpu.vcount = 0;
                        ds.gpu.end_frame();
                        if ds.c.video_buffer.should_render_this_frame() {
                            // TODO
                            let ds = &mut **ds;
                            ds.gpu.ppus[0].push_output(&mut ds.c.video_buffer);
                        }
                        ds.c.video_buffer.start_next_frame();
                    }
                    _ => (),
                }

                let vcount_match = ds.gpu.vcount.u8() == ds.gpu.dispstat.vcount();
                ds.gpu.dispstat.set_vcounter_match(vcount_match);
                if vcount_match {
                    Self::maybe_interrupt(ds, Interrupt::VCounter);
                }

                ds.gpu.dispstat.set_in_hblank(false);
                (PpuEvent::HblankStart, 960)
            }
        };

        ds.scheduler
            .schedule(NdsEvent::PpuEvent(next_event), cycles - late_by);
    }

    fn maybe_interrupt(ds: &mut Nds9, int: Interrupt) {
        if ds.gpu.dispstat.irq_enables().is_bit(int as u16) {
            Cpu::request_interrupt(ds, int);
        }
    }

    fn render_line(&mut self) {
        if self.vcount >= HEIGHT.u16() {
            return;
        }

        for ppu in &mut self.ppus {
            ppu.render_line(self.vcount);
        }
    }

    fn end_frame(&mut self) {
        for ppu in &mut self.ppus {
            ppu.end_frame();
        }
    }
}

impl Default for Gpu {
    fn default() -> Self {
        let mut gpu = Self {
            vram: Arc::new(Vram {
                a: [0; 128 * KB],
                b: [0; 128 * KB],
                c: [0; 128 * KB],
                d: [0; 128 * KB],
                e: [0; 64 * KB],
                g: [0; 16 * KB],
                h: [0; 32 * KB],
                i: [0; 16 * KB],
            }),
            ppus: [ppu::Ppu::default(), ppu::Ppu::default()],
            dispstat: DisplayStatus::default(),
            vcount: 0,
        };

        gpu.ppus[0].regs.is_a = true;
        gpu.ppus[1].regs.is_a = false;
        gpu
    }
}
