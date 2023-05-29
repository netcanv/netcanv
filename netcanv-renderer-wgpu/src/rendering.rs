use netcanv_renderer::paws::{Alignment, Color, LineCap, Point, Rect, Renderer, Vector};
use netcanv_renderer::{BlendMode, RenderBackend, ScalingFilter};

use crate::common::paws_color_to_wgpu;
use crate::WgpuBackend;

pub(crate) type ClearOps = (wgpu::Operations<wgpu::Color>, wgpu::Operations<f32>);

impl WgpuBackend {
   pub(crate) fn flush(&mut self, encoder: &mut wgpu::CommandEncoder) {
      let clear_ops = self.flush_clear();
      self.rounded_rects.flush(&self.gpu, encoder, clear_ops);
   }

   fn flush_clear(&mut self) -> ClearOps {
      if let Some(color) = self.clear.take() {
         (
            wgpu::Operations {
               load: wgpu::LoadOp::Clear(paws_color_to_wgpu(color)),
               store: true,
            },
            wgpu::Operations {
               load: wgpu::LoadOp::Clear(0.0),
               store: true,
            },
         )
      } else {
         (
            wgpu::Operations {
               load: wgpu::LoadOp::Load,
               store: true,
            },
            wgpu::Operations {
               load: wgpu::LoadOp::Load,
               store: true,
            },
         )
      }
   }
}

impl Renderer for WgpuBackend {
   type Font = Font;

   fn push(&mut self) {}

   fn pop(&mut self) {}

   fn translate(&mut self, vec: Vector) {}

   fn clip(&mut self, rect: Rect) {}

   fn fill(&mut self, rect: Rect, color: Color, radius: f32) {
      self.rounded_rects.add(self.gpu.next_depth_index(), rect, color, radius);
   }

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
      32.0
   }
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

   fn clear(&mut self, color: Color) {
      self.clear = Some(color);
   }

   fn image(&mut self, rect: Rect, image: &Self::Image) {}

   fn framebuffer(&mut self, rect: Rect, framebuffer: &Self::Framebuffer) {}

   fn scale(&mut self, scale: Vector) {}

   fn set_blend_mode(&mut self, new_blend_mode: BlendMode) {}
}

pub struct Image;

impl netcanv_renderer::Image for Image {
   fn colorized(&self, color: Color) -> Self {
      Image
   }

   fn size(&self) -> (u32, u32) {
      (24, 24)
   }
}

pub struct Framebuffer;

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (256, 256)
   }

   fn upload_rgba(&mut self, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {}

   fn download_rgba(&self, position: (u32, u32), size: (u32, u32), dest: &mut [u8]) {}

   fn set_scaling_filter(&mut self, filter: ScalingFilter) {}
}

pub struct Font;

impl netcanv_renderer::Font for Font {
   fn with_size(&self, new_size: f32) -> Self {
      Font
   }

   fn size(&self) -> f32 {
      14.0
   }

   fn height(&self) -> f32 {
      14.0
   }

   fn text_width(&self, text: &str) -> f32 {
      32.0
   }
}
