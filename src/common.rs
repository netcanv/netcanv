//! Various assorted utilities.

use netcanv_renderer::paws::{point, vector, Color, Point, Rect, Vector};

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

   /// Returns whether the point is located in the given circle.
   fn is_in_circle(self, center: Self, radius: f32) -> bool;

   /// Returns whether the point is located inside the given rectangle.
   fn is_in_rect(self, rect: Rect) -> bool;
}

impl VectorMath for Vector {
   fn floor(self) -> Self {
      vector(self.x.floor(), self.y.floor())
   }

   fn is_in_circle(self, center: Vector, radius: f32) -> bool {
      let d = self - center;
      d.x * d.x + d.y * d.y <= radius * radius
   }

   fn is_in_rect(self, rect: Rect) -> bool {
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
}

//
// Threads
//

/// A default error generated by a subsystem.
pub struct Error(pub anyhow::Error);

/// A fatal error generated by a subsystem.
///
/// Fatal errors, unlike normal errors, should kick the user out to the lobby, or perform a similar
/// action that would end the connection.
pub struct Fatal(pub anyhow::Error);

/// Catches an error onto the global bus and returns the provided value from the current function.
#[macro_export]
macro_rules! catch {
    ($exp:expr, as $T:expr, return $or:expr $(,)?) => {
        match $exp {
            Ok(ok) => ok,
            Err(err) => {
                nysa::global::push($T(::anyhow::anyhow!(err)));
                return $or
            },
        }
    };

    ($exp:expr, return $or:expr $(,)?) => {
        catch!($exp, as crate::common::Error, return $or)
    };

    ($exp:expr, as $T:expr $(,)?) => {
        catch!($exp, as $T, return ())
    };

    ($exp:expr $(,)?) => {
        catch!($exp, return ())
    };
}
