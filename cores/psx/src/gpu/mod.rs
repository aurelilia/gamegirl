// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

mod render;

use std::{iter, mem, sync::Arc};

use arrayvec::ArrayVec;
use common::{
    numutil::{NumExt, U16Ext, U32Ext},
    Colour,
};
use glow::Context;
use modular_bitfield::{
    bitfield,
    specifiers::{B1, B2, B4},
    BitfieldSpecifier,
};

use self::render::{Color, GlRender, Position};
use crate::PlayStation;

type Gp0Handler = fn(&mut Gpu, &[u32]);
type Gp0Lut = [(Gp0Handler, u8); 256];

#[bitfield]
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct GpuStat {
    texture_x_base: B4,
    texture_y_base: B1,
    semi_transparency: B2,
    texture_depth: TextureDepth,
    dither_enable: bool,
    draw_to_display: bool,
    mask_pixels: bool,
    draw_pixels: bool,
    interlace_field: bool,
    reverse_flag: bool,
    disable_textures: bool,
    horizontal_force_368: bool,
    horizontal_res: HorizontalRes,
    vertical_is_480: bool,
    is_pal: bool,
    colour_depth_24: bool,
    vertical_interlace: bool,
    disp_disable: bool,
    intr_req: bool,
    dma_req: bool,
    cmd_ready: bool,
    vram_ready: bool,
    dma_ready: bool,
    dma_dir: DmaDirection,
    interlace_is_odd: bool,
}

impl Default for GpuStat {
    fn default() -> Self {
        // todo just figure out the constant. meh
        let mut stat = Self::from(0x1C00_0000);
        stat.set_disp_disable(false);
        stat.set_vertical_interlace(true);
        stat
    }
}

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug)]
pub enum TextureDepth {
    Bit4 = 0,
    Bit8 = 1,
    Bit15 = 2,
    Reserved = 3,
}

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug)]
pub enum HorizontalRes {
    H256 = 0,
    H320 = 1,
    H512 = 2,
    H640 = 3,
}

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug)]
pub enum DmaDirection {
    Off = 0,
    Fifo = 1,
    CpuToGp0 = 2,
    VramToCpu = 3,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Gpu {
    pub stat: GpuStat,
    pub read: u32,

    rect_x_flip: bool,
    rect_y_flip: bool,
    texture_window_x_mask: u8,
    texture_window_y_mask: u8,
    texture_window_x_offs: u8,
    texture_window_y_offs: u8,
    draw_area_left: u16,
    draw_area_right: u16,
    draw_area_top: u16,
    draw_area_bottom: u16,
    draw_x_offs: i16,
    draw_y_offs: i16,
    disp_vram_x_start: u16,
    disp_vram_y_start: u16,
    disp_hori_start: u16,
    disp_hori_end: u16,
    disp_vert_start: u16,
    disp_vert_end: u16,

    gp0_cmd_buf: ArrayVec<u32, 12>,
    gp0_image_remaining: usize,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    render: Option<GlRender>,

    /// The last frame finished by the GPU, ready for display.
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_frame: Option<Vec<Colour>>,
}

impl Gpu {
    pub fn output_frame(&mut self) {
        self.render.as_mut().unwrap().draw();
    }

    pub fn gp0_write(ps: &mut PlayStation, value: u32) {
        if ps.ppu.gp0_image_remaining > 0 {
            // A texture is currently being transferred.
            Self::gp0_image_write(ps, value);
        } else {
            log::debug!("GP0 command write: {value:08X}");
            // We should run a command
            Self::gp0_command(&mut ps.ppu, value);
        }
    }

    fn gp0_command(&mut self, value: u32) {
        const GP0_CMDS: Gp0Lut = Gpu::make_gp0_table();

        self.gp0_cmd_buf.push(value);
        let (cmd, len) = GP0_CMDS[self.gp0_cmd_buf[0].bits(24, 8).us()];

        if len.us() == self.gp0_cmd_buf.len() {
            let input = self.gp0_cmd_buf.clone();
            self.gp0_cmd_buf.clear();
            (cmd)(self, &input);
        }
    }

    fn gp0_image_write(ps: &mut PlayStation, value: u32) {
        ps.ppu.gp0_image_remaining -= 1;
    }

    pub fn gp1_write(ps: &mut PlayStation, value: u32) {
        log::debug!("GP1 write: {value:08X}");
        match value.high().high() {
            0x00 => ps.ppu.gp1_reset(),
            0x01 => ps.ppu.gp1_buffer_reset(),
            0x02 => ps.ppu.gp1_ack_irq(),
            0x03 => ps.ppu.gp1_disp_enable(value),
            0x04 => ps.ppu.gp1_dma_dir(value),
            0x05 => ps.ppu.gp1_disp_vram_start(value),
            0x06 => ps.ppu.gp1_disp_hori_range(value),
            0x07 => ps.ppu.gp1_disp_vert_range(value),
            0x08 => ps.ppu.gp1_disp_mode(value),
            cmd => log::warn!("Unknown GP1 command! {cmd:X}?"),
        }
    }

    fn gp0_clear_cache(&mut self, _: &[u32]) {
        log::warn!("GPU: unimplemented: cache flush");
    }

    fn gp0_quad_mono_opaque(&mut self, input: &[u32]) {
        let positions = [
            Position::new(input[1]),
            Position::new(input[2]),
            Position::new(input[3]),
            Position::new(input[4]),
        ];
        let colors = [Color::new(input[0]); 4];
        self.render.as_mut().unwrap().add_quad(positions, colors);
    }

    fn gp0_quad_texture_opaque(&mut self, input: &[u32]) {
        let positions = [
            Position::new(input[1]),
            Position::new(input[3]),
            Position::new(input[5]),
            Position::new(input[7]),
        ];
        let colors = [Color::new(0); 4];
        self.render.as_mut().unwrap().add_quad(positions, colors);
    }

    fn gp0_tri_shaded_opaque(&mut self, input: &[u32]) {
        let positions = [
            Position::new(input[1]),
            Position::new(input[3]),
            Position::new(input[5]),
        ];
        let colors = [
            Color::new(input[0]),
            Color::new(input[2]),
            Color::new(input[4]),
        ];
        self.render.as_mut().unwrap().add_tri(positions, colors);
    }

    fn gp0_quad_shaded_opaque(&mut self, input: &[u32]) {
        let positions = [
            Position::new(input[1]),
            Position::new(input[3]),
            Position::new(input[5]),
            Position::new(input[7]),
        ];
        let colors = [
            Color::new(input[0]),
            Color::new(input[2]),
            Color::new(input[4]),
            Color::new(input[6]),
        ];
        self.render.as_mut().unwrap().add_quad(positions, colors);
    }

    fn gp0_image_load(&mut self, input: &[u32]) {
        let width = input[2].low().us();
        let height = input[2].high().us();
        // Round up
        let size = ((width * height) + 1) & !1;
        self.gp0_image_remaining = size / 2;
    }

    fn gp0_image_store(&mut self, input: &[u32]) {
        let width = input[2].low().us();
        let height = input[2].high().us();
        // Round up
        let size = (width * height).set_bit(0, false);
        log::warn!("Unhandled image store of size {size} ({height}x{width})")
    }

    fn gp0_draw_mode(&mut self, value: &[u32]) {
        let value = value[0];
        let stat: u32 = self.stat.into();
        let mut stat = GpuStat::from((stat & 0xFFFF_FC00) | value & 0x3FF);
        stat.set_disable_textures(value.is_bit(11));
        self.stat = stat;
        self.rect_x_flip = value.is_bit(12);
        self.rect_y_flip = value.is_bit(13);
    }

    fn gp0_texture_window(&mut self, value: &[u32]) {
        let value = value[0];
        self.texture_window_x_mask = value.bits(0, 5).u8();
        self.texture_window_y_mask = value.bits(5, 5).u8();
        self.texture_window_x_offs = value.bits(10, 5).u8();
        self.texture_window_y_offs = value.bits(15, 5).u8();
    }

    fn gp0_set_draw_area_tl(&mut self, value: &[u32]) {
        let value = value[0];
        self.draw_area_top = value.bits(10, 10).u16();
        self.draw_area_left = value.bits(0, 10).u16();
    }

    fn gp0_set_draw_area_br(&mut self, value: &[u32]) {
        let value = value[0];
        self.draw_area_bottom = value.bits(10, 10).u16();
        self.draw_area_right = value.bits(0, 10).u16();
    }

    fn gp0_set_draw_offset(&mut self, value: &[u32]) {
        let value = value[0];
        let x = value.bits(0, 11);
        let y = value.bits(11, 11);
        self.draw_x_offs = (x << 5) as i16 >> 5;
        self.draw_y_offs = (y << 5) as i16 >> 5;
    }

    fn gp0_set_bit_mask(&mut self, value: &[u32]) {
        let value = value[0];
        self.stat.set_mask_pixels(value.is_bit(0));
        self.stat.set_draw_pixels(value.is_bit(1));
    }

    fn gp1_reset(&mut self) {
        self.stat = GpuStat::default();
        self.rect_x_flip = false;
        self.rect_y_flip = false;

        self.texture_window_x_mask = 0;
        self.texture_window_y_mask = 0;
        self.texture_window_x_offs = 0;
        self.texture_window_y_offs = 0;
        self.draw_area_left = 0;
        self.draw_area_right = 0;
        self.draw_area_top = 0;
        self.draw_area_bottom = 0;
        self.draw_x_offs = 0;
        self.draw_y_offs = 0;
        self.disp_vram_x_start = 0;
        self.disp_vram_y_start = 0;
        self.disp_hori_start = 0x200;
        self.disp_hori_end = 0xC00;
        self.disp_vert_start = 0x10;
        self.disp_vert_end = 0x100;

        self.gp1_buffer_reset();
    }

    fn gp1_buffer_reset(&mut self) {
        self.gp0_cmd_buf.clear();
        self.gp0_image_remaining = 0;
    }

    fn gp1_ack_irq(&mut self) {
        self.stat.set_intr_req(false);
    }

    fn gp1_disp_enable(&mut self, value: u32) {
        self.stat.set_disp_disable(value.is_bit(0));
    }

    fn gp1_dma_dir(&mut self, value: u32) {
        self.stat.set_dma_dir(match value & 3 {
            0 => DmaDirection::Off,
            1 => DmaDirection::Fifo,
            2 => DmaDirection::CpuToGp0,
            _ => DmaDirection::VramToCpu,
        })
    }

    fn gp1_disp_vram_start(&mut self, value: u32) {
        self.disp_vram_x_start = value.bits(0, 10).u16() & !1;
        self.disp_vram_y_start = value.bits(10, 9).u16();
    }

    fn gp1_disp_hori_range(&mut self, value: u32) {
        self.disp_hori_start = value.bits(0, 12).u16();
        self.disp_hori_end = value.bits(12, 12).u16();
    }

    fn gp1_disp_vert_range(&mut self, value: u32) {
        self.disp_vert_start = value.bits(0, 12).u16();
        self.disp_vert_end = value.bits(12, 12).u16();
    }

    fn gp1_disp_mode(&mut self, value: u32) {
        self.stat.set_horizontal_res(match value.bits(0, 2) {
            0 => HorizontalRes::H256,
            1 => HorizontalRes::H320,
            2 => HorizontalRes::H512,
            _ => HorizontalRes::H640,
        });
        self.stat.set_vertical_is_480(value.is_bit(2));
        self.stat.set_is_pal(value.is_bit(3));
        self.stat.set_colour_depth_24(value.is_bit(4));
        self.stat.set_vertical_interlace(value.is_bit(5));
        self.stat.set_horizontal_force_368(value.is_bit(6));
        self.stat.set_reverse_flag(value.is_bit(7));
    }

    pub fn init(&mut self, ogl_ctx: Option<Arc<Context>>, ogl_tex_id: u32) {
        self.render = Some(GlRender::init(ogl_ctx.unwrap(), ogl_tex_id));
    }

    const fn make_gp0_table() -> Gp0Lut {
        let mut table: Gp0Lut =
            [(|g, buf| log::warn!("Unknown GP0 command! {:X}?", buf[0]), 1); 256];

        table[0x00] = (|_, _| (), 1);
        table[0x01] = (Gpu::gp0_clear_cache, 1);
        table[0x28] = (Gpu::gp0_quad_mono_opaque, 5);
        table[0x2C] = (Gpu::gp0_quad_texture_opaque, 9);
        table[0x30] = (Gpu::gp0_tri_shaded_opaque, 6);
        table[0x38] = (Gpu::gp0_quad_shaded_opaque, 8);
        table[0xA0] = (Gpu::gp0_image_load, 3);
        table[0xC0] = (Gpu::gp0_image_store, 3);
        table[0xE1] = (Gpu::gp0_draw_mode, 1);
        table[0xE2] = (Gpu::gp0_texture_window, 1);
        table[0xE3] = (Gpu::gp0_set_draw_area_tl, 1);
        table[0xE4] = (Gpu::gp0_set_draw_area_br, 1);
        table[0xE5] = (Gpu::gp0_set_draw_offset, 1);
        table[0xE6] = (Gpu::gp0_set_bit_mask, 1);
        table
    }
}
