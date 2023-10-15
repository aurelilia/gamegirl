use std::{mem, ptr, slice, sync::Arc};

use glow::{
    Context, HasContext, NativeBuffer, NativeFramebuffer, NativeProgram, NativeShader,
    NativeVertexArray,
};

use super::Gpu;

const VERTEX_MAX: usize = 64 * 1024;

struct Position(u16, u16);
struct Color(u8, u8, u8);

pub struct GlRender {
    gl: Arc<Context>,
    tex: u64,
    framebuffer: NativeFramebuffer,

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

    pub fn draw(&mut self) {
        unsafe {
            self.gl
                .memory_barrier(glow::CLIENT_MAPPED_BUFFER_BARRIER_BIT);
            self.gl
                .self
                .gl
                .draw_arrays(glow::TRIANGLES, 0, self.count as i32);

            let sync = self
                .gl
                .fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0)
                .unwrap();
            loop {
                let r = self
                    .gl
                    .client_wait_sync(sync, glow::SYNC_FLUSH_COMMANDS_BIT, 10000000);
                if r == glow::ALREADY_SIGNALED || r == glow::CONDITION_SATISFIED {
                    break;
                }
            }
        }
        self.count = 0;
    }

    pub fn init(gl: Arc<Context>, tex: u64) -> Self {
        unsafe fn shader(gl: &Context, src: &str, ty: u32) -> NativeShader {
            let shader = gl.create_shader(ty).unwrap();
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
            shader
        }

        unsafe {
            let vert = include_str!("vert.glsl");
            let frag = include_str!("frag.glsl");
            let vert = shader(&gl, vert, glow::VERTEX_SHADER);
            let frag = shader(&gl, frag, glow::FRAGMENT_SHADER);

            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vert);
            gl.attach_shader(program, frag);
            gl.link_program(program);

            gl.use_program(Some(program));
            let vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao));

            let positions = Buffer::new(&gl);
            let idx = gl.get_attrib_location(program, "vertex_position").unwrap();
            gl.enable_vertex_attrib_array(idx);
            gl.vertex_attrib_pointer_i32(idx, 2, glow::SHORT, 0, 0);

            let colors = Buffer::new(&gl);
            let idx = gl.get_attrib_location(program, "vertex_color").unwrap();
            gl.enable_vertex_attrib_array(idx);
            gl.vertex_attrib_pointer_i32(idx, 3, glow::UNSIGNED_BYTE, 0, 0);

            let framebuffer = gl.create_framebuffer();

            Self {
                gl,
                tex,
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
