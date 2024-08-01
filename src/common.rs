//! Various assorted utilities.

use netcanv_renderer::paws::{point, vector, Color, Point, Rect, Vector};
use netcanv_renderer::Font as FontTrait;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::backend::Font;

//
// General
//

/// Stable version of `std::ops::ControlFlow`.
pub enum ControlFlow<B> {
   Continue,
   Break(B),
}

//
// Math
//

/// Quantizes the given value, such that it lands only on multiples of `step`.
pub fn quantize(value: f32, step: f32) -> f32 {
   step * (value / step + 0.5).floor()
}

/// Performs linear interpolation between `v0` and `v1` with the provided coefficient `t`.
pub fn lerp(v0: f32, v1: f32, t: f32) -> f32 {
   (1.0 - t) * v0 + t * v1
}

/// Performs linear interpolation between two points.
pub fn lerp_point(p0: Point, p1: Point, t: f32) -> Point {
   point(lerp(p0.x, p1.x, t), lerp(p0.y, p1.y, t))
}

pub trait SafeMath {
   /// Clamps a value, automatically computing which bound is the lower one and which is the
   /// higher one.
   fn safe_clamp(self, a: Self, b: Self) -> Self;
}

impl SafeMath for f32 {
   fn safe_clamp(self, a: f32, b: f32) -> f32 {
      let min = a.min(b);
      let max = a.max(b);
      self.max(min).min(max)
   }
}

pub trait ColorMath {
   /// Returns the brightness (luma) of the color.
   fn brightness(self) -> f32;
}

impl ColorMath for Color {
   fn brightness(self) -> f32 {
      let Color { r, g, b, a } = self;
      let (r, g, b, a) = (
         r as f32 / 255.0,
         g as f32 / 255.0,
         b as f32 / 255.0,
         a as f32 / 255.0,
      );
      a * (0.2126 * r + 0.7152 * g + 0.0772 * b)
   }
}

pub trait VectorMath {
   /// Floors the vector component-wise.
   fn floor(self) -> Self;

   /// Rounds the vector component-wise.
   fn round(self) -> Self;

   /// Returns whether the point is located in the given circle.
   fn is_in_circle(&self, center: Self, radius: f32) -> bool;

   /// Returns whether the point is located inside the given rectangle.
   fn is_in_rect(&self, rect: Rect) -> bool;
}

impl VectorMath for Vector {
   fn floor(self) -> Self {
      vector(self.x.floor(), self.y.floor())
   }

   fn round(self) -> Self {
      vector(self.x.round(), self.y.round())
   }

   fn is_in_circle(&self, center: Vector, radius: f32) -> bool {
      let d = *self - center;
      d.x * d.x + d.y * d.y <= radius * radius
   }

   fn is_in_rect(&self, rect: Rect) -> bool {
      self.x >= rect.left()
         && self.y >= rect.top()
         && self.x < rect.right()
         && self.y < rect.bottom()
   }
}

/// Coordinates for four sides of a rectangle.
pub struct RectSides {
   pub left: f32,
   pub top: f32,
   pub right: f32,
   pub bottom: f32,
}

pub trait RectMath {
   /// Creates a rectangle from four sides.
   fn from_sides(sides: RectSides) -> Self;

   // Return points centered along the given side.
   fn top_center(&self) -> Point;
   fn right_center(&self) -> Point;
   fn bottom_center(&self) -> Point;
   fn left_center(&self) -> Point;

   // Sets a side of the rectangle.
   fn with_left(self, left: f32) -> Self;
   fn with_top(self, top: f32) -> Self;
   fn with_right(self, right: f32) -> Self;
   fn with_bottom(self, bottom: f32) -> Self;

   /// Sets the top-left corner of the rectangle, leaving the other corners unaffected.
   fn with_top_left(self, top_left: Point) -> Self;
   /// Sets the top-right corner of the rectangle, leaving the other corners unaffected.
   fn with_top_right(self, top_right: Point) -> Self;
   /// Sets the bottom-right corner of the rectangle, leaving the other corners unaffected.
   fn with_bottom_right(self, bottom_right: Point) -> Self;
   /// Sets the bottom-left corner of the rectangle, leaving the other corners unaffected.
   fn with_bottom_left(self, bottom_left: Point) -> Self;

   /// Returns whether the rectangle contains the given point.
   fn contains(&self, point: Point) -> bool;
}

impl RectMath for Rect {
   fn from_sides(sides: RectSides) -> Self {
      Self {
         position: point(sides.left, sides.top),
         size: vector(sides.right - sides.left, sides.bottom - sides.top),
      }
   }

   fn left_center(&self) -> Point {
      vector(self.left(), self.center_y())
   }

   fn top_center(&self) -> Point {
      vector(self.center_x(), self.top())
   }

   fn right_center(&self) -> Point {
      vector(self.right(), self.center_y())
   }

   fn bottom_center(&self) -> Point {
      vector(self.center_x(), self.bottom())
   }

   fn with_left(self, left: f32) -> Self {
      let right = self.right();
      Self::from_sides(RectSides {
         left,
         top: self.top(),
         right,
         bottom: self.bottom(),
      })
   }

   fn with_top(self, top: f32) -> Self {
      let bottom = self.bottom();
      Self::from_sides(RectSides {
         left: self.left(),
         top,
         right: self.right(),
         bottom,
      })
   }

   fn with_right(mut self, right: f32) -> Self {
      self.size.x = right - self.left();
      self
   }

   fn with_bottom(mut self, bottom: f32) -> Self {
      self.size.y = bottom - self.top();
      self
   }

   fn with_top_left(self, top_left: Point) -> Self {
      let right = self.right();
      let bottom = self.bottom();
      Self::from_sides(RectSides {
         left: top_left.x,
         top: top_left.y,
         right,
         bottom,
      })
   }

   fn with_top_right(self, top_right: Point) -> Self {
      let left = self.left();
      let bottom = self.bottom();
      Self::from_sides(RectSides {
         left,
         top: top_right.y,
         right: top_right.x,
         bottom,
      })
   }

   fn with_bottom_right(self, bottom_right: Point) -> Self {
      let left = self.left();
      let top = self.top();
      Self::from_sides(RectSides {
         left,
         top,
         right: bottom_right.x,
         bottom: bottom_right.y,
      })
   }

   fn with_bottom_left(self, bottom_left: Point) -> Self {
      let right = self.right();
      let top = self.top();
      Self::from_sides(RectSides {
         left: bottom_left.x,
         top,
         right,
         bottom: bottom_left.y,
      })
   }

   fn contains(&self, point: Point) -> bool {
      point.x >= self.left()
         && point.y >= self.top()
         && point.x <= self.right()
         && point.y <= self.bottom()
   }
}

//
// Threads
//

/// A default error generated by a subsystem.
pub struct Error(pub netcanv::Error);

/// A fatal error generated by a subsystem.
///
/// Fatal errors, unlike normal errors, should kick the user out to the lobby, or perform a similar
/// action that would end the connection.
pub struct Fatal(pub netcanv::Error);

/// A message generated by a subsystem.
///
/// Used for cases when something happened and user should be informed about this on message log.
pub struct Log(pub String);

/// Catches an error onto the global bus and returns the provided value from the current function.
#[macro_export]
macro_rules! catch {
   ($exp:expr, as $T:expr, return $or:expr $(,)?) => {
      match $exp {
         Ok(ok) => ok,
         Err(err) => {
            nysa::global::push($T(err.into()));
            return $or
         },
      }
   };

   ($exp:expr, return $or:expr $(,)?) => {
      catch!($exp, as $crate::common::Error, return $or)
   };

   ($exp:expr, as $T:expr $(,)?) => {
      catch!($exp, as $T, return ())
   };

   ($exp:expr $(,)?) => {
      catch!($exp, return ())
   };
}

//
// Text
//

/// Shrinks the given string until it matches the given width.
pub fn truncate_text(font: &Font, max_width: f32, text: &str) -> String {
   let mut text = String::from(text);
   if font.text_width(&text) > max_width {
      const ELLIPSIS: &str = "…";
      let suffix_width = font.text_width(ELLIPSIS);
      let max_width = max_width - suffix_width;
      while font.text_width(&text) > max_width {
         text.pop();
      }
      text.push_str(ELLIPSIS);
   }
   text
}

pub trait StrExt {
   fn strip_whitespace(&self) -> &str;
}

impl StrExt for &str {
   fn strip_whitespace(&self) -> &str {
      let mut start = 0;
      for (i, c) in self.char_indices() {
         if c != ' ' {
            start = i;
            break;
         }
      }
      let mut end = self.len();
      let mut last_i = self.len();
      for (i, c) in self.char_indices().rev() {
         if c != ' ' {
            end = last_i;
            break;
         }
         last_i = i;
      }
      if start > end {
         // There are no non-whitespace characters in this string.
         &self[0..0]
      } else {
         &self[start..end]
      }
   }
}

//
// (De)serialization
//

pub fn deserialize_bincode<T>(input: &[u8]) -> netcanv::Result<T>
where
   T: DeserializeOwned,
{
   bincode::deserialize(input).map_err(|e| netcanv::Error::PacketDeserializationFailed {
      error: e.to_string(),
   })
}

pub fn serialize_bincode<T>(input: &T) -> netcanv::Result<Vec<u8>>
where
   T: Serialize,
{
   bincode::serialize(input).map_err(|e| netcanv::Error::PacketSerializationFailed {
      error: e.to_string(),
   })
}
