use netcanv_renderer::paws::{Color, Rect, Vector};
use skulpin::skia_safe;

pub(crate) fn to_vector(vec: Vector) -> skia_safe::Vector {
   skia_safe::Vector::new(vec.x, vec.y)
}

pub(crate) fn to_point(vec: Vector) -> skia_safe::Point {
   skia_safe::Point::new(vec.x, vec.y)
}

pub(crate) fn to_rect(rect: Rect) -> skia_safe::Rect {
   skia_safe::Rect::from_xywh(rect.x(), rect.y(), rect.width(), rect.height())
}

pub(crate) fn to_color(color: Color) -> skia_safe::Color {
   skia_safe::Color::new(color.to_argb())
}

pub(crate) fn to_color4f(color: Color) -> skia_safe::Color4f {
   skia_safe::Color4f::new(
      color.r as f32 / 255.0,
      color.g as f32 / 255.0,
      color.b as f32 / 255.0,
      color.a as f32 / 255.0,
   )
}

pub(crate) fn rgba_image_info(width: u32, height: u32) -> skia_safe::ImageInfo {
   use skia_safe::{AlphaType, ColorType};
   skia_safe::ImageInfo::new(
      (width as i32, height as i32),
      ColorType::RGBA8888,
      AlphaType::Premul,
      None,
   )
}
