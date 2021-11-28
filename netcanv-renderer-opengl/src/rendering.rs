// Honestly, I don't like this code a lot…
// There's tons of side effects, which stem from OpenGL's statefullness.
// Most things are abstracted away such that only a few specific functions need to be called to
// draw things, so it shouldn't be _that_ horrible.

use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;

use glam::Mat3A;
use glow::HasContext;
use memoffset::offset_of;
use netcanv_renderer::paws::{
   point, vector, AlignH, AlignV, Alignment, Color, LineCap, Point, Rect, Renderer, Vector,
};
use netcanv_renderer::{
   BlendMode, Font as FontTrait, Framebuffer as FramebufferTrait, RenderBackend,
};

use crate::common::{normalized_color, to_vec2, GlUtilities, VectorMath};
use crate::font::Font;
use crate::framebuffer::Framebuffer;
use crate::image::Image;
use crate::shape_buffer::ShapeBuffer;
use crate::OpenGlBackend;

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vertex {
   pub(crate) position: Point,
   pub(crate) uv: Point,
   pub(crate) color: (f32, f32, f32, f32),
}

impl Vertex {
   fn colored(position: Point, color: Color) -> Self {
      Self {
         position,
         uv: point(0.0, 0.0),
         color: normalized_color(color),
      }
   }

   fn textured(position: Point, uv: Point) -> Self {
      Self {
         position,
         uv,
         color: (1.0, 1.0, 1.0, 1.0),
      }
   }

   fn textured_colored(position: Point, uv: Point, color: Color) -> Self {
      Self {
         position,
         uv,
         color: normalized_color(color),
      }
   }
}

struct Uniforms {
   projection: glow::UniformLocation,
   the_texture: glow::UniformLocation,
   premultiply_alpha: glow::UniformLocation,
}

#[derive(Clone, Copy, Debug)]
struct Transform {
   matrix: Mat3A,
   blend_mode: BlendMode,
   clip: Option<Rect>,
}

pub(crate) struct GlState {
   framebuffer: Option<glow::Framebuffer>,
   viewport: (u32, u32),
}

impl GlState {
   // Binds a new framebuffer, and returns the old framebuffer.
   pub(crate) fn framebuffer(
      &mut self,
      gl: &glow::Context,
      new_framebuffer: Option<glow::Framebuffer>,
   ) -> Option<glow::Framebuffer> {
      let previous_framebuffer = self.framebuffer;
      self.framebuffer = new_framebuffer;
      unsafe {
         gl.bind_framebuffer(glow::FRAMEBUFFER, self.framebuffer);
      }
      previous_framebuffer
   }

   fn viewport(&mut self, gl: &glow::Context, uniforms: &Uniforms, width: u32, height: u32) {
      let (fwidth, fheight) = (width as f32, height as f32);
      #[rustfmt::skip]
      let matrix: [f32; 3 * 3] = [
         2.0 / fwidth, 0.0,            -1.0,
         0.0,          2.0 / -fheight,  1.0,
         0.0,          0.0,             1.0,
      ];
      unsafe {
         gl.viewport(0, 0, width as i32, height as i32);
         gl.scissor(0, 0, width as i32, height as i32);
         gl.uniform_matrix_3_f32_slice(Some(&uniforms.projection), false, &matrix);
      }
      self.viewport = (width, height);
   }
}

pub(crate) struct RenderState {
   gl: Rc<glow::Context>,
   vao: glow::VertexArray,
   vbo: glow::Buffer,
   vbo_size: usize,
   ebo: glow::Buffer,
   ebo_size: usize,
   program: glow::Program,
   uniforms: Uniforms,
   null_texture: glow::Texture,
   shape: ShapeBuffer,
   stack: Vec<Transform>,
   gl_state: Rc<RefCell<GlState>>,
}

impl RenderState {
   fn create_vao(gl: &glow::Context, vbo: glow::Buffer, ebo: glow::Buffer) -> glow::VertexArray {
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

   fn create_vbo_and_ebo(gl: &glow::Context) -> (glow::Buffer, glow::Buffer) {
      unsafe {
         let vbo = gl.create_buffer().unwrap();
         let ebo = gl.create_buffer().unwrap();
         gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
         gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
         (vbo, ebo)
      }
   }

   fn compile_shader(gl: &glow::Context, kind: u32, source: &str) -> Result<glow::Shader, String> {
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

   fn create_program(gl: &glow::Context) -> (glow::Program, Uniforms) {
      const VERTEX_SHADER: &str = r#"#version 300 es

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
      const FRAGMENT_SHADER: &str = r#"#version 300 es

         precision mediump float;

         in vec2 vertex_uv;
         in vec4 vertex_color;

         uniform sampler2D the_texture;
         uniform float premultiply_alpha;

         out vec4 fragment_color;

         void main(void)
         {
            vec4 color = vertex_color * texture(the_texture, vertex_uv);
            float alpha_factor = premultiply_alpha * color.a + (1.0 - premultiply_alpha);
            color.rgb *= alpha_factor;
            fragment_color = color;
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
            the_texture: gl.get_uniform_location(program, "the_texture").unwrap(),
            premultiply_alpha: gl.get_uniform_location(program, "premultiply_alpha").unwrap(),
         };
         gl.uniform_1_i32(Some(&uniforms.the_texture), 0);
         gl.uniform_1_f32(Some(&uniforms.premultiply_alpha), 0.0);

         (program, uniforms)
      }
   }

   fn create_null_texture(gl: &glow::Context) -> glow::Texture {
      unsafe {
         let texture = gl.create_texture().unwrap();
         gl.bind_texture(glow::TEXTURE_2D, Some(texture));
         gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            1,
            1,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            Some(&[255, 255, 255, 255]),
         );
         texture
      }
   }

   pub(crate) fn new(gl: Rc<glow::Context>) -> Self {
      let (vbo, ebo) = Self::create_vbo_and_ebo(&gl);
      let vao = Self::create_vao(&gl, vbo, ebo);
      let (program, uniforms) = Self::create_program(&gl);
      let null_texture = Self::create_null_texture(&gl);

      unsafe {
         gl.enable(glow::BLEND);
         gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
         gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
      }

      let transform = Transform {
         matrix: Mat3A::IDENTITY,
         blend_mode: BlendMode::Alpha,
         clip: None,
      };

      let mut state = Self {
         gl,
         vao,
         vbo,
         vbo_size: 0,
         ebo,
         ebo_size: 0,
         program,
         uniforms,
         null_texture,
         stack: vec![transform],
         shape: ShapeBuffer::new(),
         gl_state: Rc::new(RefCell::new(GlState {
            framebuffer: None,
            viewport: (0, 0),
         })),
      };
      state.apply_transform();
      state
   }

   unsafe fn to_u8_slice<T>(slice: &[T]) -> &[u8] {
      let ptr = slice.as_ptr() as *const u8;
      std::slice::from_raw_parts(ptr, size_of::<T>() * slice.len())
   }

   fn bind_null_texture(&mut self) {
      unsafe {
         self.gl.active_texture(glow::TEXTURE0);
         self.gl.bind_texture(glow::TEXTURE_2D, Some(self.null_texture));
      }
   }

   fn draw(&mut self) {
      unsafe {
         // Update buffers
         let vertex_data = Self::to_u8_slice(&self.shape.vertices);
         let index_data = Self::to_u8_slice(&self.shape.indices);
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
         self.gl.draw_elements(
            glow::TRIANGLES,
            self.shape.indices.len() as i32,
            glow::UNSIGNED_INT,
            0,
         );
      }
   }

   pub(crate) fn viewport(&mut self, width: u32, height: u32) {
      self.gl_state.borrow_mut().viewport(&self.gl, &self.uniforms, width, height);
   }

   fn transform(&self) -> &Transform {
      self.stack.last().unwrap()
   }

   fn transform_mut(&mut self) -> &mut Transform {
      self.stack.last_mut().unwrap()
   }

   fn apply_transform(&mut self) {
      let transform = self.transform();
      let mut premultiply_alpha = false;
      match transform.blend_mode {
         BlendMode::Clear => unsafe {
            self.gl.blend_equation(glow::FUNC_ADD);
            self.gl.blend_func(glow::ZERO, glow::ZERO);
         },
         BlendMode::Alpha => unsafe {
            self.gl.blend_equation(glow::FUNC_ADD);
            self.gl.blend_func_separate(
               glow::SRC_ALPHA,
               glow::ONE_MINUS_SRC_ALPHA,
               glow::ONE,
               glow::ONE_MINUS_SRC_ALPHA,
            );
         },
         BlendMode::Add => unsafe {
            self.gl.blend_equation(glow::FUNC_ADD);
            self.gl.blend_func(glow::SRC_ALPHA, glow::ONE);
         },
         BlendMode::Invert => unsafe {
            self.gl.blend_equation(glow::FUNC_ADD);
            self.gl.blend_func_separate(
               glow::ONE_MINUS_DST_COLOR,
               glow::ONE_MINUS_SRC_ALPHA,
               glow::ZERO,
               glow::ONE,
            );
            premultiply_alpha = true;
         },
      }
      unsafe {
         self.gl.uniform_1_f32(
            Some(&self.uniforms.premultiply_alpha),
            premultiply_alpha as i32 as f32,
         );
         if let Some(clip_rect) = &transform.clip {
            let viewport = self.gl_state.borrow().viewport;
            let top_left = clip_rect.top_left();
            let bottom_right = clip_rect.bottom_right();
            let (width, height) = (bottom_right.x - top_left.x, bottom_right.y - top_left.y);
            let y = viewport.1 as f32 - top_left.y - height;
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(top_left.x as i32, y as i32, width as i32, height as i32);
         } else {
            self.gl.disable(glow::SCISSOR_TEST);
         }
      }
   }
}

fn text_origin(rect: &Rect, font: &Font, text: &str, alignment: Alignment) -> Point {
   let x = match alignment.0 {
      AlignH::Left => rect.left(),
      AlignH::Center => rect.center_x() - font.text_width(text) / 2.0,
      AlignH::Right => rect.right() - font.text_width(text),
   };
   let y = match alignment.1 {
      AlignV::Top => rect.top() + font.height(),
      AlignV::Middle => rect.center_y() + font.height() / 2.0,
      AlignV::Bottom => rect.bottom(),
   };
   point(x.floor(), y.floor())
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

impl OpenGlBackend {
   fn start(&mut self) {
      self.state.shape.start(self.state.transform().matrix);
   }

   fn shape(&mut self) -> &mut ShapeBuffer {
      &mut self.state.shape
   }

   fn point(&mut self, point: Point, radius: f32, color: Color, style: LineCap) {
      match style {
         LineCap::Butt => (),
         LineCap::Square | LineCap::Round => self.fill(
            Rect::new(
               point - vector(radius, radius),
               vector(radius * 2.0, radius * 2.0),
            ),
            color,
            if style == LineCap::Round { radius } else { 0.0 },
         ),
      }
   }
}

impl Renderer for OpenGlBackend {
   type Font = Font;

   fn push(&mut self) {
      self.state.stack.push(self.state.transform().clone());
   }

   fn pop(&mut self) {
      self.state.stack.pop();
      assert!(
         self.state.stack.len() > 0,
         "pop() called at the bottom of the stack"
      );
      self.state.apply_transform();
   }

   fn translate(&mut self, vec: Vector) {
      self.state.transform_mut().matrix *= Mat3A::from_translation(to_vec2(vec));
   }

   fn clip(&mut self, rect: Rect) {
      self.state.transform_mut().clip = Some(rect);
      self.state.apply_transform();
   }

   fn fill(&mut self, rect: Rect, color: Color, radius: f32) {
      use std::f32::consts::PI;

      self.state.bind_null_texture();
      self.start();
      if radius > 0.0 {
         let inner_rect = Rect::new(
            rect.position + vector(radius, radius),
            rect.size - vector(radius, radius) * 2.0,
         );
         let (inner_top_left, inner_top_right, inner_bottom_right, inner_bottom_left) =
            self.shape().rect(
               Vertex::colored(inner_rect.top_left(), color),
               Vertex::colored(inner_rect.bottom_right(), color),
            );
         // Top edge
         let top_left = self.shape().push_vertex(Vertex::colored(
            rect.top_left() + vector(radius, 0.0),
            color,
         ));
         let top_right = self.shape().push_vertex(Vertex::colored(
            rect.top_right() + vector(-radius, 0.0),
            color,
         ));
         self.shape().quad_indices(top_left, top_right, inner_top_right, inner_top_left);
         // Right edge
         let right_upper = self.shape().push_vertex(Vertex::colored(
            rect.top_right() + vector(0.0, radius),
            color,
         ));
         let right_lower = self.shape().push_vertex(Vertex::colored(
            rect.bottom_right() + vector(0.0, -radius),
            color,
         ));
         self.shape().quad_indices(
            inner_top_right,
            right_upper,
            right_lower,
            inner_bottom_right,
         );
         // Bottom edge
         let bottom_left = self.shape().push_vertex(Vertex::colored(
            rect.bottom_left() + vector(radius, 0.0),
            color,
         ));
         let bottom_right = self.shape().push_vertex(Vertex::colored(
            rect.bottom_right() + vector(-radius, 0.0),
            color,
         ));
         self.shape().quad_indices(
            inner_bottom_left,
            inner_bottom_right,
            bottom_right,
            bottom_left,
         );
         // Left edge
         let left_upper = self.shape().push_vertex(Vertex::colored(
            rect.top_left() + vector(0.0, radius),
            color,
         ));
         let left_lower = self.shape().push_vertex(Vertex::colored(
            rect.bottom_left() + vector(0.0, -radius),
            color,
         ));
         self.shape().quad_indices(left_upper, inner_top_left, inner_bottom_left, left_lower);
         // Corners
         self.shape().arc(inner_top_left, inner_rect.top_left(), radius, PI, 1.5 * PI);
         self.shape().arc(
            inner_top_right,
            inner_rect.top_right(),
            radius,
            1.5 * PI,
            2.0 * PI,
         );
         self.shape().arc(
            inner_bottom_right,
            inner_rect.bottom_right(),
            radius,
            0.0,
            0.5 * PI,
         );
         self.shape().arc(
            inner_bottom_left,
            inner_rect.bottom_left(),
            radius,
            0.5 * PI,
            PI,
         );
         self.state.draw();
      } else {
         self.shape().rect(
            Vertex::colored(rect.top_left(), color),
            Vertex::colored(rect.bottom_right(), color),
         );
         self.state.draw();
      }
   }

   fn outline(&mut self, mut rect: Rect, color: Color, radius: f32, thickness: f32) {
      use std::f32::consts::PI;

      self.start();

      if thickness % 2.0 > 0.95 {
         rect.position += vector(0.5, 0.5);
         rect.size -= vector(1.0, 1.0);
      }
      let d = thickness / 2.0;
      if radius > 0.0 {
         // Top edge
         self.shape().rect(
            Vertex::colored(rect.top_left() + vector(radius, -d), color),
            Vertex::colored(rect.top_right() + vector(-radius, d), color),
         );
         // Right edge
         self.shape().rect(
            Vertex::colored(rect.top_right() + vector(-d, radius), color),
            Vertex::colored(rect.bottom_right() + vector(d, -radius), color),
         );
         // Bottom edge
         self.shape().rect(
            Vertex::colored(rect.bottom_left() + vector(radius, -d), color),
            Vertex::colored(rect.bottom_right() + vector(-radius, d), color),
         );
         // Left edge
         self.shape().rect(
            Vertex::colored(rect.top_left() + vector(-d, radius), color),
            Vertex::colored(rect.bottom_left() + vector(d, -radius), color),
         );

         let vertex_template = Vertex::colored(vector(0.0, 0.0), color);
         // Top left corner
         self.shape().arc_outline(
            rect.top_left() + vector(radius, radius),
            vertex_template,
            radius,
            thickness,
            PI,
            1.5 * PI,
         );
         // Top right corner
         self.shape().arc_outline(
            rect.top_right() + vector(-radius, radius),
            vertex_template,
            radius,
            thickness,
            1.5 * PI,
            2.0 * PI,
         );
         // Bottom right corner
         self.shape().arc_outline(
            rect.bottom_right() + vector(-radius, -radius),
            vertex_template,
            radius,
            thickness,
            0.0,
            0.5 * PI,
         );
         // Bottom left corner
         self.shape().arc_outline(
            rect.bottom_left() + vector(radius, -radius),
            vertex_template,
            radius,
            thickness,
            0.5 * PI,
            PI,
         );
      } else {
         let outer_top_left =
            self.shape().push_vertex(Vertex::colored(rect.top_left() - vector(d, d), color));
         let inner_top_left =
            self.shape().push_vertex(Vertex::colored(rect.top_left() + vector(d, d), color));
         let outer_top_right =
            self.shape().push_vertex(Vertex::colored(rect.top_right() - vector(-d, d), color));
         let inner_top_right =
            self.shape().push_vertex(Vertex::colored(rect.top_right() + vector(-d, d), color));
         let outer_bottom_right =
            self.shape().push_vertex(Vertex::colored(rect.bottom_right() - vector(-d, -d), color));
         let inner_bottom_right =
            self.shape().push_vertex(Vertex::colored(rect.bottom_right() + vector(-d, -d), color));
         let outer_bottom_left =
            self.shape().push_vertex(Vertex::colored(rect.bottom_left() - vector(d, -d), color));
         let inner_bottom_left =
            self.shape().push_vertex(Vertex::colored(rect.bottom_left() + vector(d, -d), color));
         // Top edge
         self.shape().quad_indices(
            outer_top_left,
            outer_top_right,
            inner_top_right,
            inner_top_left,
         );
         // Right edge
         self.shape().quad_indices(
            outer_top_right,
            inner_top_right,
            inner_bottom_right,
            outer_bottom_right,
         );
         // Bottom edge
         self.shape().quad_indices(
            outer_bottom_left,
            outer_bottom_right,
            inner_bottom_right,
            inner_bottom_left,
         ); // Left edge
         self.shape().quad_indices(
            outer_top_left,
            inner_top_left,
            inner_bottom_left,
            outer_bottom_left,
         );
      }
      self.state.bind_null_texture();
      self.state.draw();
   }

   fn line(&mut self, mut a: Point, mut b: Point, color: Color, cap: LineCap, thickness: f32) {
      use std::f32::consts::PI;

      let half_thickness = thickness / 2.0;
      if a == b {
         self.point(a, half_thickness, color, cap);
         return;
      }

      if thickness % 2.0 > 0.95 {
         a += vector(0.5, 0.5);
         b += vector(0.5, 0.5);
      }

      let direction = (b - a).normalize();
      if cap == LineCap::Square {
         a -= direction * half_thickness;
         b += direction * half_thickness;
      }
      let cw = direction.perpendicular_cw() * half_thickness;
      let ccw = direction.perpendicular_ccw() * half_thickness;
      self.start();
      self.shape().quad(
         Vertex::colored(a + cw, color),
         Vertex::colored(a + ccw, color),
         Vertex::colored(b + ccw, color),
         Vertex::colored(b + cw, color),
      );
      if cap == LineCap::Round {
         let angle = direction.y.atan2(direction.x);
         let angle_cw = angle + PI / 2.0;
         let angle_ccw = angle - PI / 2.0;
         let a_index = self.shape().push_vertex(Vertex::colored(a, color));
         let b_index = self.shape().push_vertex(Vertex::colored(b, color));
         self.shape().arc(a_index, a, half_thickness, angle_cw, angle_cw + PI);
         self.shape().arc(b_index, b, half_thickness, angle_ccw, angle_ccw + PI);
      }

      self.state.bind_null_texture();
      self.state.draw();
   }

   fn text(
      &mut self,
      rect: Rect,
      font: &Font,
      text: &str,
      color: Color,
      alignment: Alignment,
   ) -> f32 {
      // Set up textures.
      unsafe {
         let atlas = font.atlas();
         self.gl.active_texture(glow::TEXTURE0);
         self.gl.bind_texture(glow::TEXTURE_2D, Some(atlas));
      }

      // Buffer up the glyphs.
      let origin = text_origin(&rect, font, text, alignment);
      self.start();
      for (mut position, uv) in font.typeset(text) {
         position.position += origin;
         self.shape().rect(
            Vertex::textured_colored(position.top_left(), uv.top_left(), color),
            Vertex::textured_colored(position.bottom_right(), uv.bottom_right(), color),
         );
      }

      // Draw 'em.
      self.state.draw();
      0.0
   }
}

impl RenderBackend for OpenGlBackend {
   type Image = Image;

   type Framebuffer = Framebuffer;

   fn create_image_from_rgba(&mut self, width: u32, height: u32, pixel_data: &[u8]) -> Self::Image {
      Image::from_rgba(Rc::clone(&self.gl), width, height, pixel_data)
   }

   fn create_font_from_memory(&mut self, data: &[u8], default_size: f32) -> Self::Font {
      Font::new(
         Rc::clone(&self.gl),
         Rc::clone(&self.freetype),
         data,
         default_size,
      )
   }

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer {
      Framebuffer::new(
         Rc::clone(&self.gl),
         Rc::clone(&self.state.gl_state),
         width,
         height,
      )
   }

   fn draw_to(&mut self, framebuffer: &Framebuffer, f: impl FnOnce(&mut Self)) {
      let previous_framebuffer;
      let previous_viewport;
      {
         let mut gl_state = self.state.gl_state.borrow_mut();
         previous_framebuffer = gl_state.framebuffer(&self.gl, Some(framebuffer.framebuffer()));
         previous_viewport = gl_state.viewport;
         gl_state.viewport(
            &self.gl,
            &self.state.uniforms,
            framebuffer.width(),
            framebuffer.height(),
         );
      }
      f(self);
      let mut gl_state = self.state.gl_state.borrow_mut();
      gl_state.framebuffer(&self.gl, previous_framebuffer);
      let (width, height) = previous_viewport;
      gl_state.viewport(&self.gl, &self.state.uniforms, width, height);
   }

   fn clear(&mut self, color: Color) {
      let (r, g, b, a) = normalized_color(color);
      unsafe {
         self.gl.clear_color(r, g, b, a);
         self.gl.clear(glow::COLOR_BUFFER_BIT);
      }
   }

   fn image(&mut self, rect: Rect, image: &Image) {
      let color = image.color.unwrap_or(Color::WHITE);
      self.start();
      self.shape().rect(
         Vertex::textured_colored(rect.top_left(), point(0.0, 0.0), color),
         Vertex::textured_colored(rect.bottom_right(), point(1.0, 1.0), color),
      );
      unsafe {
         self.gl.active_texture(glow::TEXTURE0);
         self.gl.bind_texture(glow::TEXTURE_2D, Some(image.texture.texture));
         let swizzle_mask = if image.color.is_some() {
            [glow::ONE, glow::ONE, glow::ONE, glow::ALPHA]
         } else {
            [glow::RED, glow::GREEN, glow::BLUE, glow::ALPHA]
         };
         self.gl.texture_swizzle_mask(glow::TEXTURE_2D, &swizzle_mask);
         self.state.draw();
      }
   }

   fn framebuffer(&mut self, rect: Rect, framebuffer: &Framebuffer) {
      assert!(
         self.state.gl_state.borrow().framebuffer != Some(framebuffer.framebuffer()),
         "cannot render a framebuffer to itself"
      );
      self.start();
      self.shape().rect(
         Vertex::textured(rect.top_left(), point(0.0, 1.0)),
         Vertex::textured(rect.bottom_right(), point(1.0, 0.0)),
      );
      let texture = framebuffer.texture();
      unsafe {
         self.gl.active_texture(glow::TEXTURE0);
         self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
         self.state.draw();
      }
   }

   fn scale(&mut self, scale: Vector) {
      self.state.transform_mut().matrix *= Mat3A::from_scale(to_vec2(scale));
   }

   fn set_blend_mode(&mut self, new_blend_mode: BlendMode) {
      self.state.transform_mut().blend_mode = new_blend_mode;
      self.state.apply_transform();
   }
}
