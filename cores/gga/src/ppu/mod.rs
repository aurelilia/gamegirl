// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! For this PPU implementation, I took a lot of reference from DenSinH's GBAC-.
//! It is not an outright copy, but I want to thank them for their code
//! that helped me understand the PPU's more complex behavior.
//! The code is under the MIT license at https://github.com/DenSinH/GBAC-.
//! Additionally RustBoyAdvance-ng by michelhe was heavily used for the
//! second attempt at an implementation. Thank you to michelhe, too.
//! The code is under the MIT license at  https://github.com/michelhe/rustboyadvance-ng.

pub mod registers;
mod render;

use alloc::sync::Arc;

use armchair::Interrupt;
use common::{common::video::FrameBuffer, numutil::NumExt, Colour};
use registers::*;
use render::{PpuRender, PpuRendererKind};

use crate::{
    cpu::GgaFullBus,
    hw::dma::{Dmas, Reason},
    memory::KB,
    scheduling::{AdvEvent, PpuEvent},
};

const WIDTH: usize = 240;
const HEIGHT: usize = 160;
const VBLANK_END: u16 = 228;
const BLACK: Colour = [0, 0, 0, 255];
const WHITE: Colour = [31, 31, 31, 255];
const TRANS: Colour = [0, 0, 0, 0];

#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Point(i32, i32);

impl Point {
    fn inbounds(self, w: usize, h: usize) -> bool {
        let Point(x, y) = self;
        x >= 0 && x < w as i32 && y >= 0 && y < h as i32
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Ppu {
    // Registers
    pub regs: PpuRegisters,

    // Memory
    pub palette: Arc<[u8]>,
    pub vram: Arc<[u8]>,
    pub oam: Arc<[u8]>,

    // Renderer
    #[cfg_attr(feature = "serde", serde(skip, default))]
    render: PpuRendererKind,
}

impl Ppu {
    pub fn handle_event(gg: &mut GgaFullBus<'_>, event: PpuEvent, late_by: i64) {
        let (next_event, cycles) = match event {
            PpuEvent::HblankStart => {
                if gg.c.video_buffer.should_render_this_frame() {
                    gg.ppu.render_line();
                }

                Self::maybe_interrupt(gg, Interrupt::HBlank);
                Dmas::update_all(gg, Reason::HBlank);
                (PpuEvent::SetHblank, 46)
            }

            PpuEvent::SetHblank => {
                gg.ppu.regs.dispstat.set_in_hblank(true);
                (PpuEvent::HblankEnd, 226)
            }

            PpuEvent::HblankEnd => {
                gg.ppu.regs.vcount += 1;

                let vcount = gg.ppu.regs.vcount;
                match () {
                    _ if gg.ppu.regs.vcount == HEIGHT.u16() => {
                        gg.ppu.regs.dispstat.set_in_vblank(true);
                        Self::maybe_interrupt(gg, Interrupt::VBlank);
                        Dmas::update_all(gg, Reason::VBlank);
                    }
                    // VBlank flag gets set one scanline early
                    _ if vcount == (VBLANK_END - 1) => {
                        gg.ppu.regs.dispstat.set_in_vblank(false);
                    }
                    _ if vcount == VBLANK_END => {
                        gg.ppu.regs.vcount = 0;
                        gg.ppu.end_frame();
                        if gg.c.video_buffer.should_render_this_frame() {
                            gg.bus.ppu.push_output(&mut gg.bus.c.video_buffer);
                        }
                        gg.c.video_buffer.start_next_frame();
                    }
                    _ => (),
                }

                let vcount_match = gg.ppu.regs.vcount.u8() == gg.ppu.regs.dispstat.vcount();
                gg.ppu.regs.dispstat.set_vcounter_match(vcount_match);
                if vcount_match {
                    Self::maybe_interrupt(gg, Interrupt::VCounter);
                }

                gg.ppu.regs.dispstat.set_in_hblank(false);
                (PpuEvent::HblankStart, 960)
            }
        };

        gg.scheduler
            .schedule(AdvEvent::PpuEvent(next_event), cycles - late_by);
    }

    fn maybe_interrupt(gg: &mut GgaFullBus<'_>, int: Interrupt) {
        if gg.ppu.regs.dispstat.irq_enables().is_bit(int as u16) {
            gg.cpu.request_interrupt(gg.bus, int);
        }
    }

    fn render_line(&mut self) {
        if self.regs.vcount >= HEIGHT.u16() {
            return;
        }

        self.render.do_line(self.regs.clone());

        // Update affines
        for bg in 2..4 {
            if self.regs.bg_enabled(bg.u16()) {
                self.regs.bg_scale[bg - 2].latched.0 += self.regs.bg_scale[bg - 2].pb as i32;
                self.regs.bg_scale[bg - 2].latched.1 += self.regs.bg_scale[bg - 2].pd as i32;
            }
        }
    }

    fn end_frame(&mut self) {
        // Reload affine backgrounds
        self.regs.bg_scale[0].latch();
        self.regs.bg_scale[1].latch();
    }

    fn push_output(&mut self, buf: &mut FrameBuffer) {
        if let Some(last_frame) = self.render.get_last() {
            buf.push(last_frame);
        }
    }

    pub fn init_render(gg: &mut GgaFullBus<'_>) {
        let render = PpuRender::new(
            Arc::clone(&gg.ppu.palette),
            Arc::clone(&gg.ppu.vram),
            Arc::clone(&gg.ppu.oam),
        );
        gg.ppu.render = PpuRendererKind::new(render, gg.c.config.threaded_ppu);
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            regs: Default::default(),

            palette: Arc::new([0; KB]),
            vram: Arc::new([0; 96 * KB]),
            oam: Arc::new([0; KB]),

            render: PpuRendererKind::Invalid,
        }
    }
}
