use netcanv_renderer::paws::{vector, Color, Rect, Vector};

pub fn normalized_color(color: Color) -> (f32, f32, f32, f32) {
   (
      color.r as f32 / 255.0,
      color.g as f32 / 255.0,
      color.b as f32 / 255.0,
      color.a as f32 / 255.0,
   )
}

pub trait VectorMath {
   fn length(self) -> f32;
   fn normalize(self) -> Self;
   fn perpendicular_cw(self) -> Self;
   fn perpendicular_ccw(self) -> Self;
}

impl VectorMath for Vector {
   fn length(self) -> f32 {
      (self.x * self.x + self.y * self.y).sqrt()
   }

   fn normalize(self) -> Self {
      let length = self.length();
      if length == 0.0 {
         vector(0.0, 0.0)
      } else {
         self / length
      }
   }

   fn perpendicular_cw(self) -> Self {
      vector(-self.y, self.x)
   }

   fn perpendicular_ccw(self) -> Self {
      vector(self.y, -self.x)
   }
}

pub trait RectMath {
   fn uv(self, texture_size: Vector) -> Self;
}

impl RectMath for Rect {
   fn uv(self, texture_size: Vector) -> Self {
      Rect::new(self.position / texture_size, self.size / texture_size)
   }
}
