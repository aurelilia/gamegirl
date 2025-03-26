// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::sync::Arc;

use arm_cpu::{Cpu, Interrupt};
use capture::CaptureUnit;
use common::{numutil::NumExt, UnsafeArc};
use engine3d::Engine3D;
use modular_bitfield::prelude::*;
use ppu::{registers::DisplayStatus, Ppu, HEIGHT, VBLANK_END};
use vram::{Vram, VramCtrl};

use crate::{
    hw::dma::{Dmas, Reason},
    memory::KB,
    scheduling::{NdsEvent, PpuEvent},
    CpuDevice, Nds, Nds9, NdsCpu,
};

mod capture;
mod engine3d;
mod ppu;
pub mod vram;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Gpu {
    pub vram: UnsafeArc<Vram>,
    pub ppus: [ppu::Ppu; 2],
    pub gpu: Engine3D,
    pub capture: CaptureUnit,

    pub(super) dispstat: CpuDevice<DisplayStatus>,
    pub(super) powcnt1: PowerControl1,
    pub(super) vcount: u16,
}

impl Gpu {
    pub fn handle_event(ds: &mut Nds, event: PpuEvent, late_by: i64) {
        // This is... ugly.
        let (next_event, cycles) = match event {
            PpuEvent::HblankStart => {
                if ds.c.video_buffer.should_render_this_frame() {
                    ds.gpu.render_line();
                }

                Self::maybe_interrupt(&mut ds.nds7(), Interrupt::HBlank);
                Self::maybe_interrupt(&mut ds.nds9(), Interrupt::HBlank);
                Dmas::update_all(&mut ds.nds7(), Reason::HBlank);
                Dmas::update_all(&mut ds.nds9(), Reason::HBlank);
                (PpuEvent::SetHblank, 46) // TODO timing here
            }

            PpuEvent::SetHblank => {
                ds.gpu.dispstat[0].set_in_hblank(true);
                ds.gpu.dispstat[1].set_in_hblank(true);
                (PpuEvent::HblankEnd, 1142)
            }

            PpuEvent::HblankEnd => {
                ds.gpu.vcount += 1;

                let vcount = ds.gpu.vcount;
                match () {
                    _ if ds.gpu.vcount == HEIGHT.u16() => {
                        ds.gpu.dispstat[0].set_in_vblank(true);
                        ds.gpu.dispstat[1].set_in_vblank(true);
                        Self::maybe_interrupt(&mut ds.nds7(), Interrupt::VBlank);
                        Self::maybe_interrupt(&mut ds.nds9(), Interrupt::VBlank);
                        Dmas::update_all(&mut ds.nds7(), Reason::VBlank);
                        Dmas::update_all(&mut ds.nds9(), Reason::VBlank);
                    }
                    // VBlank flag gets set one scanline early
                    _ if vcount == (VBLANK_END - 1) => {
                        ds.gpu.dispstat[0].set_in_vblank(false);
                        ds.gpu.dispstat[1].set_in_vblank(false);
                    }
                    _ if vcount == VBLANK_END => {
                        ds.gpu.vcount = 0;
                        ds.gpu.end_frame();
                        if ds.c.video_buffer.should_render_this_frame() {
                            Self::push_output(ds);
                        }
                        ds.c.video_buffer.start_next_frame();
                    }
                    _ => (),
                }

                let vcount_match = ds.gpu.vcount.u8() == ds.gpu.dispstat[0].vcount();
                ds.gpu.dispstat[0].set_vcounter_match(vcount_match);
                if vcount_match {
                    Self::maybe_interrupt(&mut ds.nds7(), Interrupt::VCounter);
                }
                let vcount_match = ds.gpu.vcount.u8() == ds.gpu.dispstat[1].vcount();
                ds.gpu.dispstat[1].set_vcounter_match(vcount_match);
                if vcount_match {
                    Self::maybe_interrupt(&mut ds.nds9(), Interrupt::VCounter);
                }

                ds.gpu.dispstat[0].set_in_hblank(false);
                ds.gpu.dispstat[1].set_in_hblank(false);
                (PpuEvent::HblankStart, 3072)
            }
        };

        ds.scheduler
            .schedule(NdsEvent::PpuEvent(next_event), cycles - late_by);
    }

    pub fn init_render(ds: &mut Nds) {
        Ppu::init_render(ds);
    }

    fn push_output(ds: &mut Nds) {
        let Some(mut a) = ds.gpu.ppus[0].get_output() else {
            return;
        };
        let Some(mut b) = ds.gpu.ppus[1].get_output() else {
            return;
        };
        a.append(&mut b);
        ds.c.video_buffer.push(a);
    }

    fn maybe_interrupt<DS: NdsCpu>(ds: &mut DS, int: Interrupt) {
        if ds.gpu.dispstat[DS::I].irq_enables().is_bit(int as u16) {
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
            vram: UnsafeArc::new(Vram::default()),
            ppus: [ppu::Ppu::default(), ppu::Ppu::default()],
            gpu: Engine3D::default(),
            capture: CaptureUnit::default(),
            dispstat: [DisplayStatus::default(); 2],
            powcnt1: PowerControl1::default(),
            vcount: 0,
        };

        gpu.ppus[0].regs.is_a = true;
        gpu.ppus[1].regs.is_a = false;
        gpu
    }
}

#[bitfield]
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PowerControl1 {
    pub disp_en: bool,
    pub ppu_engine_a: bool,
    pub render_engine: bool,
    pub geom_engine: bool,
    #[skip]
    __: B5,
    pub ppu_engine_b: bool,
    #[skip]
    __: B5,
    pub disp_swap: bool,
    #[skip]
    __: B16,
}
