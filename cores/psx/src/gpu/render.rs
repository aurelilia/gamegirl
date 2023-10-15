use std::{mem, ptr, slice, sync::Arc};

use common::numutil::{NumExt, U16Ext, U32Ext};
use glow::{
    Context, HasContext, NativeBuffer, NativeFramebuffer, NativeProgram, NativeShader,
    NativeTexture, NativeVertexArray,
};

use super::Gpu;

const VERTEX_MAX: usize = 64 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct Position(pub i16, pub i16);

impl Position {
    pub fn new(input: u32) -> Self {
        Self(input.low() as i16, input.high() as i16)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub fn new(input: u32) -> Self {
        Self(input.low().low(), input.low().high(), input.high().low())
    }
}

pub struct GlRender {
    gl: Arc<Context>,
    tex: NativeTexture,
    fbo: NativeFramebuffer,

    program: NativeProgram,
    vertex: NativeShader,
    fragment: NativeShader,
    vao: NativeVertexArray,

    positions: Buffer<Position>,
    colors: Buffer<Color>,
    count: usize,
}

impl GlRender {
    pub fn add_tri(&mut self, pos: [Position; 3], col: [Color; 3]) {
        for i in 0..3 {
            self.positions.content[self.count] = pos[i];
            self.colors.content[self.count] = col[i];
            self.count += 1;
        }
    }

    pub fn add_quad(&mut self, pos: [Position; 4], col: [Color; 4]) {
        for i in 0..3 {
            self.positions.content[self.count] = pos[i];
            self.colors.content[self.count] = col[i];
            self.count += 1;
        }
        for i in 1..4 {
            self.positions.content[self.count] = pos[i];
            self.colors.content[self.count] = col[i];
            self.count += 1;
        }
    }

    pub fn draw(&mut self) {
        log::warn!("Drawing {} vertices", self.count);
        unsafe {
            self.gl.use_program(Some(self.program));
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.viewport(0, 0, 1024, 512);
            self.gl.enable(glow::BLEND);
            self.gl.blend_equation(glow::FUNC_ADD);
            self.gl.blend_func(glow::ONE, glow::ZERO);

            self.gl
                .memory_barrier(glow::CLIENT_MAPPED_BUFFER_BARRIER_BIT);
            self.gl.draw_arrays(glow::TRIANGLES, 0, self.count as i32);

            self.gl.use_program(None);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.gl.bind_vertex_array(None);
        }
        self.count = 0;
    }

    pub fn init(gl: Arc<Context>, tex: u32) -> Self {
        unsafe fn shader(gl: &Context, src: &str, ty: u32) -> NativeShader {
            let shader = gl.create_shader(ty).unwrap();
            gl.shader_source(shader, src);
            gl.compile_shader(shader);

            if !gl.get_shader_compile_status(shader) {
                log::error!(
                    "Failed to compile shader!\n{}",
                    gl.get_shader_info_log(shader)
                );
                panic!();
            }

            shader
        }

        unsafe {
            let tex = NativeTexture(tex.try_into().unwrap());
            let vert = include_str!("vert.glsl");
            let frag = include_str!("frag.glsl");
            let vert = shader(&gl, vert, glow::VERTEX_SHADER);
            let frag = shader(&gl, frag, glow::FRAGMENT_SHADER);

            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vert);
            gl.attach_shader(program, frag);
            gl.link_program(program);
            assert!(gl.get_program_link_status(program));

            gl.use_program(Some(program));
            let vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao));

            let positions = Buffer::new(&gl);
            let idx = gl.get_attrib_location(program, "vertex_position").unwrap();
            gl.vertex_attrib_pointer_i32(idx, 2, glow::SHORT, 4, 0);
            gl.enable_vertex_attrib_array(idx);

            let colors = Buffer::new(&gl);
            let idx = gl.get_attrib_location(program, "vertex_color").unwrap();
            gl.vertex_attrib_pointer_i32(idx, 3, glow::UNSIGNED_BYTE, 3, 0);
            gl.enable_vertex_attrib_array(idx);

            let fbo = gl.create_framebuffer().unwrap();
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGB as i32,
                1024,
                512,
                0,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                None,
            );
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(tex),
                0,
            );
            assert_eq!(
                gl.check_framebuffer_status(glow::FRAMEBUFFER),
                glow::FRAMEBUFFER_COMPLETE
            );

            gl.use_program(None);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_vertex_array(None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            Self {
                gl,
                tex,
                fbo,
                program,
                vertex: vert,
                fragment: frag,
                vao,
                positions,
                colors,
                count: 0,
            }
        }
    }
}

impl Drop for GlRender {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_shader(self.vertex);
            self.gl.delete_shader(self.fragment);
            self.gl.delete_program(self.program);
        }
    }
}

struct Buffer<T: 'static> {
    ctx: Arc<Context>,
    gl: NativeBuffer,
    content: &'static mut [T],
}

impl<T> Buffer<T> {
    fn new(gl: &Arc<Context>) -> Self {
        unsafe {
            let buffer = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));

            let size = (VERTEX_MAX * mem::size_of::<T>()) as i32;
            let access = glow::MAP_WRITE_BIT | glow::MAP_PERSISTENT_BIT;

            gl.buffer_storage(glow::ARRAY_BUFFER, size, None, access);
            let mem = gl.map_buffer_range(glow::ARRAY_BUFFER, 0, size, access);

            {
                let mut content = slice::from_raw_parts_mut(mem, size as usize);
                content.fill(0);
            }

            let content = slice::from_raw_parts_mut(mem as *mut T, size as usize);
            Self {
                ctx: gl.clone(),
                gl: buffer,
                content,
            }
        }
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe {
            self.ctx.bind_buffer(glow::ARRAY_BUFFER, Some(self.gl));
            self.ctx.unmap_buffer(glow::ARRAY_BUFFER);
            self.ctx.delete_buffer(self.gl);
        }
    }
}

impl Gpu {}
