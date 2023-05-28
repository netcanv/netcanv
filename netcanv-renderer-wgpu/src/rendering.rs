use netcanv_renderer::paws::{Alignment, Color, LineCap, Point, Rect, Renderer, Vector};
use netcanv_renderer::{BlendMode, RenderBackend, ScalingFilter};

use crate::WgpuBackend;

pub struct Font;

impl netcanv_renderer::Font for Font {
   fn with_size(&self, new_size: f32) -> Self {
      Font
   }

   fn size(&self) -> f32 {
      0.0
   }

   fn height(&self) -> f32 {
      0.0
   }

   fn text_width(&self, text: &str) -> f32 {
      0.0
   }
}

impl Renderer for WgpuBackend {
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

pub struct Image;

impl netcanv_renderer::Image for Image {
   fn colorized(&self, color: Color) -> Self {
      Image
   }

   fn size(&self) -> (u32, u32) {
      (0, 0)
   }
}

pub struct Framebuffer;

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (0, 0)
   }

   fn upload_rgba(&mut self, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {}

   fn download_rgba(&self, position: (u32, u32), size: (u32, u32), dest: &mut [u8]) {}

   fn set_scaling_filter(&mut self, filter: ScalingFilter) {}
}

impl RenderBackend for WgpuBackend {
   type Image = Image;

   type Framebuffer = Framebuffer;

   fn create_image_from_rgba(&mut self, width: u32, height: u32, pixel_data: &[u8]) -> Self::Image {
      Image
   }

   fn create_font_from_memory(&mut self, data: &[u8], default_size: f32) -> Self::Font {
      Font
   }

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer {
      Framebuffer
   }

   fn draw_to(&mut self, framebuffer: &Self::Framebuffer, f: impl FnOnce(&mut Self)) {}

   fn clear(&mut self, color: Color) {}

   fn image(&mut self, rect: Rect, image: &Self::Image) {}

   fn framebuffer(&mut self, rect: Rect, framebuffer: &Self::Framebuffer) {}

   fn scale(&mut self, scale: Vector) {}

   fn set_blend_mode(&mut self, new_blend_mode: BlendMode) {}
}
