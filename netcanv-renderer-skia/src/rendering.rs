use paws::{vector, Alignment, Color, LineCap, Point, Rect, Renderer, SizedImage, Vector};
use skulpin::skia_safe::{
   self,
   paint::{Cap, Style},
   ClipOp, Paint,
};

use crate::SkiaBackend;

pub struct Font {}

impl netcanv_renderer::Font for Font {
   fn height(&self) -> f32 {
      14.0
   }

   fn text_width(&self, text: &str) -> f32 {
      0.0
   }

   fn from_memory(memory: &[u8], default_size: f32) -> Self {
      Self {}
   }
}

pub struct Image {
   image: skia_safe::Image,
}

impl SizedImage for Image {
   fn size(&self) -> Vector {
      vector(self.image.width() as f32, self.image.height() as f32)
   }
}

impl netcanv_renderer::Image for Image {
   fn from_rgba(width: usize, height: usize, pixel_data: &[u8]) -> Self {
      todo!()
   }
}

impl SkiaBackend {
   fn draw_rect(&mut self, rect: Rect, radius: f32, paint: &Paint) {
      let rect = to_rect(rect);
      if radius <= 0.0 {
         self.canvas().draw_rect(rect, &paint);
      } else {
         let rrect = skia_safe::RRect::new_rect_xy(rect, radius, radius);
         self.canvas().draw_rrect(rrect, &paint);
      }
   }
}

impl Renderer for SkiaBackend {
   type Font = Font;
   type Image = Image;

   fn push(&mut self) {
      self.canvas().save();
   }

   fn pop(&mut self) {
      self.canvas().restore();
   }

   fn translate(&mut self, vec: Vector) {
      self.canvas().translate(to_point(vec));
   }

   fn clip(&mut self, rect: Rect) {
      self.canvas().clip_rect(to_rect(rect), ClipOp::Intersect, false);
   }

   fn fill(&mut self, rect: Rect, color: Color, radius: f32) {
      let paint = Paint::new(to_color4f(color), None);
      self.draw_rect(rect, radius, &paint);
   }

   fn outline(&mut self, rect: Rect, color: Color, radius: f32, thickness: f32) {
      let mut paint = Paint::new(to_color4f(color), None);
      paint.set_style(Style::Stroke);
      paint.set_stroke_width(thickness);
      self.draw_rect(rect, radius, &paint);
   }

   fn line(&mut self, a: Point, b: Point, color: Color, cap: LineCap, thickness: f32) {
      let mut paint = Paint::new(to_color4f(color), None);
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
   ) {
      // TODO: text.
   }

   fn image(&mut self, rect: Rect, image: &Self::Image) {
      // TODO: images.
   }
}

fn to_point(vec: Vector) -> skia_safe::Point {
   skia_safe::Point::new(vec.x, vec.y)
}

fn to_rect(rect: Rect) -> skia_safe::Rect {
   skia_safe::Rect::from_xywh(rect.x(), rect.y(), rect.width(), rect.height())
}

fn to_color4f(color: Color) -> skia_safe::Color4f {
   skia_safe::Color4f::new(
      color.r as f32,
      color.g as f32,
      color.b as f32,
      color.a as f32,
   )
}
