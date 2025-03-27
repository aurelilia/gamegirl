// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::ops::RangeInclusive;

use common::{
    numutil::{hword, NumExt},
    Colour, UnsafeArc,
};
use objects::ObjPixel;

use super::{BackgroundMode, DisplayMode, PpuRegisters, HEIGHT, TRANS, WIDTH};
use crate::graphics::Vram;

// #[cfg(feature = "std")]
extern crate std;

mod modes;
mod objects;
mod palette;
mod pixels;
// pub mod threading;
mod tile;

fn xy2d(x: usize, y: usize) -> usize {
    xy2dw(x, y, WIDTH)
}

fn xy2dw(x: usize, y: usize, w: usize) -> usize {
    (y * w) + x
}

const BUF: usize = WIDTH * HEIGHT;

type Layer = [Colour; WIDTH];

#[derive(Default)]
pub enum PpuRendererKind {
    #[default]
    Invalid,
    SingleCore(Box<PpuRender>),
    #[cfg(feature = "std")]
    Threaded {
        sender: std::sync::mpsc::Sender<PpuRegisters>,
        last: std::sync::Arc<std::sync::Mutex<Option<Vec<Colour>>>>,
    },
}

impl PpuRendererKind {
    pub fn get_last(&self) -> Option<Vec<Colour>> {
        match self {
            PpuRendererKind::SingleCore(s) => Some(s.pixels.to_vec()),
            #[cfg(feature = "std")]
            PpuRendererKind::Threaded { last, .. } => last.lock().unwrap().take(),
            PpuRendererKind::Invalid => unreachable!(),
        }
    }

    pub fn do_line(&mut self, regs: PpuRegisters) {
        match self {
            PpuRendererKind::SingleCore(s) => {
                s.r = regs;
                s.render_line()
            }
            #[cfg(feature = "std")]
            PpuRendererKind::Threaded { sender, .. } => sender.send(regs).unwrap(),
            PpuRendererKind::Invalid => unreachable!(),
        }
    }

    pub fn new(mut render: PpuRender, is_multi: bool) -> Self {
        #[cfg(feature = "std")]
        if is_multi {
            let (sender, rx) = std::sync::mpsc::channel();
            let last = Arc::new(std::sync::Mutex::new(None));

            let last_mutex = Arc::clone(&last);
            std::thread::spawn(move || loop {
                let Ok(regs) = rx.recv() else { return };
                render.r = regs;
                render.render_line();

                if render.r.vcount == (HEIGHT.u16() - 1) {
                    *last_mutex.lock().unwrap() = Some(render.pixels.to_vec());
                }
            });

            return Self::Threaded { sender, last };
        }
        Self::SingleCore(Box::new(render))
    }
}

pub struct PpuRender {
    // PPU state
    r: PpuRegisters,
    pub palette: Arc<[u8]>,
    pub vram: UnsafeArc<Vram>,
    pub oam: Arc<[u8]>,

    /// Pixels of the frame currently being constructed.
    pub pixels: [Colour; BUF],
    /// Pixel output of each background layer.
    bg_layers: [Layer; 4],
    /// Pixel output of the object layer.
    obj_layer: [ObjPixel; WIDTH],
}

impl PpuRender {
    fn render_line(&mut self) {
        if self.r.dispcnt.forced_blank_enable() {
            self.forced_blank();
            return;
        }

        match self.r.dispcnt.display_mode() {
            DisplayMode::DisplayOff => self.forced_blank(),

            DisplayMode::Normal => {
                if self.r.dispcnt.obj_en() {
                    self.render_objs();
                }

                match self.r.dispcnt.bg_mode() {
                    BackgroundMode::Mode0 => self.render_mode0(),
                    BackgroundMode::Mode1 => self.render_mode1(),
                    BackgroundMode::Mode2 => self.render_mode2(),
                    BackgroundMode::Mode3 => self.render_mode3(),
                    BackgroundMode::Mode4 => self.render_mode4(),
                    BackgroundMode::Mode5 => self.render_mode5(),
                    BackgroundMode::Mode6 if self.r.is_a => self.render_mode6(),
                    inv => log::warn!("Invalid PPU mode {inv:?}"),
                }
            }

            DisplayMode::VramDisplay => {
                let block = self.r.dispcnt.vram_block();
                let ram = &self.vram.v[block.us()];
                let start = self.r.vcount.us() * WIDTH;
                for pixel in 0..WIDTH {
                    let idx = start + pixel;
                    let vram_addr = idx << 1;
                    let color = hword(ram[vram_addr], ram[vram_addr + 1]);
                    let color = [
                        color.bits(0, 5).u8(),
                        color.bits(5, 5).u8(),
                        color.bits(10, 5).u8(),
                        255,
                    ];
                    self.pixels[start + pixel] = Self::bit5_to_bit8(color);
                }
            }

            DisplayMode::MemoryDisplay => todo!(),
        }
    }

    fn forced_blank(&mut self) {
        let start = self.r.vcount.us() * WIDTH;
        for pixel in 0..WIDTH {
            self.pixels[start + pixel] = [255, 255, 255, 255];
        }
        return;
    }

    fn clean_buffers(&mut self, bgs: RangeInclusive<usize>) {
        for bg in bgs {
            self.bg_layers[bg] = [TRANS; WIDTH];
        }
        self.obj_layer = serde_obj_arr();
    }

    fn bg_vram(&self, addr: usize) -> u8 {
        let offs = (!self.r.is_a) as usize * 0x20_0000;
        self.vram.get9(offs + addr).unwrap_or(0)
    }

    fn obj_vram(&self, addr: usize) -> u8 {
        let offs = (!self.r.is_a) as usize * 0x20_0000;
        self.vram.get9(0x40_0000 + offs + addr).unwrap_or(0)
    }

    pub fn new(palette: Arc<[u8]>, vram: UnsafeArc<Vram>, oam: Arc<[u8]>) -> Self {
        Self {
            r: PpuRegisters::default(),
            palette,
            vram,
            oam,

            pixels: serde_colour_arr(),
            bg_layers: serde_layer_arr(),
            obj_layer: serde_obj_arr(),
        }
    }
}

fn serde_colour_arr() -> [Colour; BUF] {
    [TRANS; BUF]
}
fn serde_layer_arr() -> [Layer; 4] {
    [[TRANS; WIDTH]; 4]
}
fn serde_obj_arr() -> [ObjPixel; WIDTH] {
    [ObjPixel::default(); WIDTH]
}
