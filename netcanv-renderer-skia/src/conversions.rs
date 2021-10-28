use paws::{Color, Rect, Vector};
use skulpin::skia_safe;

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
