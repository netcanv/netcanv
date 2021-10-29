// Honestly, I don't like this code a lot…
// There's tons of side effects, which stem from OpenGL's statefullness.
// Most things are abstracted away such that only a few specific functions need to be called to
// draw things, so it shouldn't be _that_ horrible.

use std::mem::size_of;
use std::rc::Rc;

use glow::{
   HasContext, NativeBuffer, NativeProgram, NativeShader, NativeTexture, NativeUniformLocation,
   NativeVertexArray,
};
use memoffset::offset_of;
use netcanv_renderer::paws::{point, Alignment, Color, LineCap, Point, Rect, Renderer, Vector};
use netcanv_renderer::RenderBackend;

use crate::common::normalized_color;
use crate::font::Font;
use crate::framebuffer::Framebuffer;
use crate::image::Image;
use crate::OpenGlBackend;

#[repr(packed)]
pub(crate) struct Vertex {
   position: Point,
   uv: Point,
   color: (f32, f32, f32, f32),
}

impl From<Point> for Vertex {
   fn from(position: Point) -> Self {
      Self {
         position,
         uv: point(0.0, 0.0),
         color: (1.0, 1.0, 1.0, 1.0),
      }
   }
}

struct Uniforms {
   projection: NativeUniformLocation,
}

pub(crate) struct RenderState {
   gl: Rc<glow::Context>,
   vao: NativeVertexArray,
   vbo: NativeBuffer,
   vbo_size: usize,
   ebo: NativeBuffer,
   ebo_size: usize,
   program: NativeProgram,
   uniforms: Uniforms,
}

impl RenderState {
   fn create_vao(gl: &glow::Context, vbo: NativeBuffer, ebo: NativeBuffer) -> NativeVertexArray {
      unsafe {
         let vao = gl.create_vertex_array().unwrap();
         gl.bind_vertex_array(Some(vao));
         gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
         gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
         let stride = size_of::<Vertex>() as i32;
         gl.vertex_attrib_pointer_f32(
            0,                                   // index
            2,                                   // size
            glow::FLOAT,                         // type
            false,                               // normalize
            stride,                              // stride
            offset_of!(Vertex, position) as i32, // offset
         );
         gl.vertex_attrib_pointer_f32(
            1,                             // index
            2,                             // size
            glow::FLOAT,                   // type
            false,                         // normalize
            stride,                        // stride
            offset_of!(Vertex, uv) as i32, // offset
         );
         gl.vertex_attrib_pointer_f32(
            2,                                // index
            4,                                // size
            glow::FLOAT,                      // type
            false,                            // normalize
            stride,                           // stride
            offset_of!(Vertex, color) as i32, // offset
         );
         gl.enable_vertex_attrib_array(0);
         gl.enable_vertex_attrib_array(1);
         gl.enable_vertex_attrib_array(2);
         vao
      }
   }

   fn create_vbo_and_ebo(gl: &glow::Context) -> (NativeBuffer, NativeBuffer) {
      unsafe {
         let vbo = gl.create_buffer().unwrap();
         let ebo = gl.create_buffer().unwrap();
         gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
         gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
         (vbo, ebo)
      }
   }

   fn compile_shader(gl: &glow::Context, kind: u32, source: &str) -> Result<NativeShader, String> {
      unsafe {
         let shader = gl.create_shader(kind)?;
         gl.shader_source(shader, source);
         gl.compile_shader(shader);
         if !gl.get_shader_compile_status(shader) {
            Err(gl.get_shader_info_log(shader))
         } else {
            Ok(shader)
         }
      }
   }

   fn create_program(gl: &glow::Context) -> (NativeProgram, Uniforms) {
      const VERTEX_SHADER: &str = r#"
         #version 300 es

         precision mediump float;

         layout (location = 0) in vec2 position;
         layout (location = 1) in vec2 uv;
         layout (location = 2) in vec4 color;

         uniform mat3 projection;

         out vec2 vertex_uv;
         out vec4 vertex_color;

         void main(void)
         {
            vec3 transformed_position = vec3(position, 1.0) * projection;
            gl_Position = vec4(transformed_position, 1.0);
            vertex_uv = uv;
            vertex_color = color;
         }
      "#;
      const FRAGMENT_SHADER: &str = r#"
         #version 300 es

         precision mediump float;

         in vec2 vertex_uv;
         in vec4 vertex_color;

         out vec4 fragment_color;

         void main(void)
         {
            fragment_color = vec4(1.0, 1.0, 1.0, 1.0);
         }
      "#;
      unsafe {
         let vertex_shader = Self::compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER).unwrap();
         let fragment_shader =
            Self::compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER).unwrap();

         gl.shader_source(vertex_shader, VERTEX_SHADER);
         gl.compile_shader(vertex_shader);
         gl.shader_source(fragment_shader, FRAGMENT_SHADER);
         gl.compile_shader(fragment_shader);

         let program = gl.create_program().unwrap();
         gl.attach_shader(program, vertex_shader);
         gl.attach_shader(program, fragment_shader);
         gl.link_program(program);

         gl.delete_shader(vertex_shader);
         gl.delete_shader(fragment_shader);

         gl.use_program(Some(program));

         let uniforms = Uniforms {
            projection: gl.get_uniform_location(program, "projection").unwrap(),
         };

         (program, uniforms)
      }
   }

   pub(crate) fn new(gl: Rc<glow::Context>) -> Self {
      let (vbo, ebo) = Self::create_vbo_and_ebo(&gl);
      let vao = Self::create_vao(&gl, vbo, ebo);
      let (program, uniforms) = Self::create_program(&gl);
      Self {
         gl,
         vao,
         vbo,
         vbo_size: 0,
         ebo,
         ebo_size: 0,
         program,
         uniforms,
      }
   }

   unsafe fn to_u8_slice<T>(slice: &[T]) -> &[u8] {
      let ptr = slice.as_ptr() as *const u8;
      std::slice::from_raw_parts(ptr, size_of::<T>() * slice.len())
   }

   pub(crate) fn draw(&mut self, vertices: &[Vertex], indices: &[u32]) {
      unsafe {
         // Update buffers
         let vertex_data = Self::to_u8_slice(vertices);
         let index_data = Self::to_u8_slice(indices);
         if vertex_data.len() > self.vbo_size {
            self.gl.buffer_data_size(
               glow::ARRAY_BUFFER,
               vertex_data.len() as i32,
               glow::STREAM_DRAW,
            );
            self.vbo_size = vertex_data.len();
         }
         if index_data.len() > self.ebo_size {
            self.gl.buffer_data_size(
               glow::ELEMENT_ARRAY_BUFFER,
               index_data.len() as i32,
               glow::STREAM_DRAW,
            );
            self.ebo_size = index_data.len();
         }
         self.gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, vertex_data);
         self.gl.buffer_sub_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, 0, index_data);
         // Draw triangles
         self.gl.draw_elements(glow::TRIANGLES, indices.len() as i32, glow::UNSIGNED_INT, 0);
      }
   }

   pub(crate) fn viewport(&mut self, width: u32, height: u32) {
      let (fwidth, fheight) = (width as f32, height as f32);
      #[rustfmt::skip]
      let matrix: [f32; 3 * 3] = [
         2.0 / fwidth, 0.0,            -1.0,
         0.0,          2.0 / -fheight, 1.0,
         0.0,          0.0,            1.0,
      ];
      unsafe {
         self.gl.viewport(0, 0, width as i32, height as i32);
         self.gl.scissor(0, 0, width as i32, height as i32);
         self.gl.uniform_matrix_3_f32_slice(Some(&self.uniforms.projection), false, &matrix);
      }
   }
}

impl Drop for RenderState {
   fn drop(&mut self) {
      unsafe {
         self.gl.delete_buffer(self.vbo);
         self.gl.delete_buffer(self.ebo);
         self.gl.delete_vertex_array(self.vao);
         self.gl.delete_program(self.program);
      }
   }
}

impl Renderer for OpenGlBackend {
   type Font = Font;

   fn push(&mut self) {}

   fn pop(&mut self) {}

   fn translate(&mut self, vec: Vector) {}

   fn clip(&mut self, rect: Rect) {}

   fn fill(&mut self, rect: Rect, color: Color, radius: f32) {}

   fn outline(&mut self, rect: Rect, color: Color, radius: f32, thickness: f32) {}

   fn line(&mut self, a: Point, b: Point, color: Color, cap: LineCap, thickness: f32) {}

   fn text(
      &mut self,
      rect: Rect,
      font: &Self::Font,
      text: &str,
      color: Color,
      alignment: Alignment,
   ) -> f32 {
      0.0
   }
}

impl RenderBackend for OpenGlBackend {
   type Image = Image;

   type Framebuffer = Framebuffer;

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer {
      Framebuffer {}
   }

   fn draw_to(&mut self, framebuffer: &Self::Framebuffer, f: impl FnOnce(&mut Self)) {}

   fn clear(&mut self, color: Color) {
      let (r, g, b, a) = normalized_color(color);
      unsafe {
         self.gl.clear_color(r, g, b, a);
         self.gl.clear(glow::COLOR_BUFFER_BIT);
      }
   }

   fn image(&mut self, position: Point, image: &Self::Image) {}

   fn framebuffer(&mut self, position: Point, framebuffer: &Self::Framebuffer) {}

   fn scale(&mut self, scale: Vector) {}

   fn set_blend_mode(&mut self, new_blend_mode: netcanv_renderer::BlendMode) {}
}
