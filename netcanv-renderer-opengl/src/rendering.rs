use netcanv_renderer::paws::{Alignment, Color, LineCap, Point, Rect, Renderer, Vector};
use netcanv_renderer::RenderBackend;

use crate::font::Font;
use crate::framebuffer::Framebuffer;
use crate::image::Image;
use crate::OpenGlBackend;

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

   fn clear(&mut self, color: Color) {}

   fn image(&mut self, position: Point, image: &Self::Image) {}

   fn framebuffer(&mut self, position: Point, framebuffer: &Self::Framebuffer) {}

   fn scale(&mut self, scale: Vector) {}

   fn set_blend_mode(&mut self, new_blend_mode: netcanv_renderer::BlendMode) {}
}
