use std::cell::Cell;

use netcanv_renderer::{BlendMode, Font as FontTrait, RenderBackend};
use paws::{vector, AlignH, AlignV, Alignment, Color, LineCap, Point, Rect, Renderer, Vector};
use skulpin::skia_safe::{
   self, color_filters, image_filters,
   paint::{Cap, Style},
   AlphaType, ClipOp, ColorType, Data, IRect, ISize, ImageInfo, Paint, SamplingOptions, Surface,
   Typeface,
};

use crate::conversions::*;
use crate::SkiaBackend;

/// A wrapper for Skia fonts.
pub struct Font {
   font: skia_safe::Font,
   height_in_pixels: f32,
}

impl Font {
   fn from_skia_font(font: skia_safe::Font) -> Self {
      Self {
         height_in_pixels: font.metrics().1.cap_height.abs(),
         font,
      }
   }
}

impl netcanv_renderer::Font for Font {
   fn from_memory(memory: &[u8], default_size: f32) -> Self {
      let typeface =
         Typeface::from_data(Data::new_copy(memory), None).expect("failed to load typeface");
      let font = skia_safe::Font::new(typeface, default_size);
      Self::from_skia_font(font)
   }

   fn with_size(&self, new_size: f32) -> Self {
      let font = self.font.with_size(new_size).expect("cannot create font with negative size");
      Self::from_skia_font(font)
   }

   fn size(&self) -> f32 {
      self.font.size()
   }

   fn height(&self) -> f32 {
      self.height_in_pixels
   }

   fn text_width(&self, text: &str) -> f32 {
      let (advance, _) = self.font.measure_str(text, None);
      advance
   }
}

/// An image.
pub struct Image {
   pub(crate) image: skia_safe::Image,
}

impl netcanv_renderer::Image for Image {
   fn from_rgba(width: usize, height: usize, pixel_data: &[u8]) -> Self {
      let image = skia_safe::Image::from_raster_data(
         &ImageInfo::new_s32((width as i32, height as i32), AlphaType::Premul),
         Data::new_copy(pixel_data),
         width * 4,
      )
      .expect("failed to create the image");
      Self { image }
   }

   fn colorized(&self, color: Color) -> Self {
      let image_bounds = IRect::new(0, 0, self.image.width(), self.image.height());
      let color_filter =
         color_filters::blend(to_color(color), skia_safe::BlendMode::SrcATop).unwrap();
      let filter = image_filters::color_filter(color_filter, None, None).unwrap();
      let colored_image =
         self.image.new_with_filter(None, &filter, image_bounds, image_bounds).unwrap().0;
      Image {
         image: colored_image,
      }
   }

   fn size(&self) -> (usize, usize) {
      (self.image.width() as usize, self.image.height() as usize)
   }
}

pub struct Framebuffer {
   surface: Cell<Option<Surface>>,
}

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn upload_rgba(&mut self, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {
      todo!()
   }

   fn download_rgba(&self, dest: &mut [u8]) {
      todo!()
   }
}

impl SkiaBackend {
   fn create_paint(&self, color: Color) -> Paint {
      let mut paint = Paint::new(to_color4f(color), None);
      paint.set_blend_mode(match self.stack.last().unwrap().blend_mode {
         BlendMode::Clear => skia_safe::BlendMode::Clear,
         BlendMode::Alpha => skia_safe::BlendMode::SrcOver,
         BlendMode::Add => skia_safe::BlendMode::Plus,
         BlendMode::Subtract => skia_safe::BlendMode::Difference,
      });
      paint
   }

   /// Helper function for drawing rectangles with the given paint.
   fn draw_rect(&mut self, rect: Rect, radius: f32, mut paint: Paint) {
      let rect = to_rect(rect);
      if radius > 0.0 {
         paint.set_anti_alias(true);
      }
      if radius <= 0.0 {
         self.canvas().draw_rect(rect, &paint);
      } else {
         let rrect = skia_safe::RRect::new_rect_xy(rect, radius, radius);
         self.canvas().draw_rrect(rrect, &paint);
      }
   }

   /// Returns the origin (bottom left corner) of the text, with the given layout parameters.
   fn text_origin(
      &self,
      rect: &Rect,
      font: &Font,
      text: &str,
      alignment: Alignment,
   ) -> (Point, f32) {
      let text_width = font.text_width(text);
      let text_height = font.height();
      let x = match alignment.0 {
         AlignH::Left => rect.left(),
         AlignH::Center => rect.center_x() - text_width / 2.0,
         AlignH::Right => rect.right(),
      };
      let y = match alignment.1 {
         AlignV::Top => rect.top() + text_height,
         AlignV::Middle => rect.center_y() + text_height / 2.0,
         AlignV::Bottom => rect.bottom(),
      };
      (vector(x, y), text_width)
   }
}

impl Renderer for SkiaBackend {
   type Font = Font;

   fn push(&mut self) {
      self.stack.push(self.stack.last().unwrap().clone());
      self.canvas().save();
   }

   fn pop(&mut self) {
      self.stack.pop();
      assert!(
         self.stack.len() > 0,
         "pop() called at the bottom of the stack"
      );
      self.canvas().restore();
   }

   fn translate(&mut self, vec: Vector) {
      self.canvas().translate(to_point(vec));
   }

   fn clip(&mut self, rect: Rect) {
      self.canvas().clip_rect(to_rect(rect), ClipOp::Intersect, false);
   }

   fn fill(&mut self, rect: Rect, color: Color, radius: f32) {
      let paint = self.create_paint(color);
      self.draw_rect(rect, radius, paint);
   }

   fn outline(&mut self, mut rect: Rect, color: Color, radius: f32, thickness: f32) {
      let mut paint = self.create_paint(color);
      paint.set_style(Style::Stroke);
      paint.set_stroke_width(thickness);
      if thickness % 2.0 >= 0.95 {
         rect.position += vector(0.5, 0.5);
      }
      self.draw_rect(rect, radius, paint);
   }

   fn line(&mut self, a: Point, b: Point, color: Color, cap: LineCap, thickness: f32) {
      let mut paint = self.create_paint(color);
      paint.set_style(Style::Stroke);
      paint.set_stroke_width(thickness);
      paint.set_stroke_cap(match cap {
         LineCap::Butt => Cap::Butt,
         LineCap::Square => Cap::Square,
         LineCap::Round => Cap::Round,
      });
      self.canvas().draw_line(to_point(a), to_point(b), &paint);
   }

   fn text(
      &mut self,
      rect: Rect,
      font: &Self::Font,
      text: &str,
      color: Color,
      alignment: Alignment,
   ) -> f32 {
      let (origin, advance) = self.text_origin(&rect, font, text, alignment);
      let mut paint = self.create_paint(color);
      paint.set_anti_alias(true);
      self.canvas().draw_str(text, to_point(origin), &font.font, &paint);
      advance
   }
}

impl RenderBackend for SkiaBackend {
   type Image = Image;
   type Framebuffer = Framebuffer;

   fn create_framebuffer(&mut self, width: usize, height: usize) -> Framebuffer {
      let image_info = ImageInfo::new(
         ISize::new(width as i32, height as i32),
         ColorType::RGBA8888,
         AlphaType::Premul,
         None,
      );
      let surface = self
         .canvas()
         .new_surface(&image_info, None)
         .expect("failed to create framebuffer surface");
      Framebuffer {
         surface: Cell::new(Some(surface)),
      }
   }

   fn draw_to(&mut self, framebuffer: &Framebuffer, f: impl FnOnce(&mut Self)) {
      let surface_outer = framebuffer.surface.take();
      let surface = surface_outer.as_ref().unwrap();
      let old_surface = self.surface.inner.replace(surface.clone());
      f(self);
      self.surface.inner.replace(old_surface.unwrap());
      framebuffer.surface.set(surface_outer);
   }

   fn clear(&mut self, color: Color) {
      self.canvas().clear(to_color(color));
   }

   fn image(&mut self, point: Point, image: &Image) {
      self.canvas().draw_image(&image.image, to_point(point), None);
   }

   fn framebuffer(&mut self, position: Point, framebuffer: &Framebuffer) {
      // The skia_safe devs were out of their fucking mind when they pulled that one.
      // Drawing a surface to a canvas requires the surface to be mutable.
      // I'm speechless.
      let mut surface_outer = framebuffer.surface.take();
      let surface = surface_outer.as_mut().unwrap();
      surface.draw(
         self.canvas(),
         to_point(position),
         SamplingOptions::default(),
         None,
      );
      framebuffer.surface.set(surface_outer);
   }

   fn scale(&mut self, scale: Vector) {
      self.canvas().scale((scale.x, scale.y));
   }

   fn set_blend_mode(&mut self, new_blend_mode: BlendMode) {
      self.stack.last_mut().unwrap().blend_mode = new_blend_mode;
   }
}
