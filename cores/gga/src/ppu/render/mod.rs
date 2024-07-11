// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{
    ops::RangeInclusive,
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};

use common::{numutil::NumExt, Colour};
use objects::ObjPixel;

use super::{BackgroundMode, PpuRegisters, HEIGHT, TRANS, WIDTH};

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
    Threaded {
        sender: Sender<PpuRegisters>,
        last: Arc<Mutex<Option<Vec<Colour>>>>,
    },
}

impl PpuRendererKind {
    pub fn get_last(&self) -> Option<Vec<Colour>> {
        match self {
            PpuRendererKind::SingleCore(s) => Some(s.pixels.to_vec()),
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
            PpuRendererKind::Threaded { sender, .. } => sender.send(regs).unwrap(),
            PpuRendererKind::Invalid => unreachable!(),
        }
    }

    pub fn new(mut render: PpuRender, is_multi: bool) -> Self {
        if is_multi {
            let (sender, rx) = mpsc::channel();
            let last = Arc::new(Mutex::new(None));

            let last_mutex = Arc::clone(&last);
            thread::spawn(move || loop {
                let Ok(regs) = rx.recv() else { return };
                render.r = regs;
                render.render_line();

                if render.r.vcount == (HEIGHT.u16() - 1) {
                    *last_mutex.lock().unwrap() = Some(render.pixels.to_vec());
                }
            });

            Self::Threaded { sender, last }
        } else {
            Self::SingleCore(Box::new(render))
        }
    }
}

pub struct PpuRender {
    // PPU state
    r: PpuRegisters,
    pub palette: Arc<[u8]>,
    pub vram: Arc<[u8]>,
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
            let start = self.r.vcount.us() * WIDTH;
            for pixel in 0..WIDTH {
                self.pixels[start + pixel] = [31, 31, 31, 255];
            }
            return;
        }

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
            inv => log::warn!("Invalid PPU mode {inv:?}"),
        }
    }

    fn clean_buffers(&mut self, bgs: RangeInclusive<usize>) {
        for bg in bgs {
            self.bg_layers[bg] = [TRANS; WIDTH];
        }
        self.obj_layer = serde_obj_arr();
    }

    pub fn new(palette: Arc<[u8]>, vram: Arc<[u8]>, oam: Arc<[u8]>) -> Self {
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
