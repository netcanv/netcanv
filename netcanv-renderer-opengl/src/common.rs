use glam::Vec2;
use glow::HasContext;
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

pub trait GlUtilities {
   unsafe fn texture_swizzle_mask(&self, target: u32, mask: &[u32; 4]);
}

impl GlUtilities for glow::Context {
   unsafe fn texture_swizzle_mask(&self, target: u32, mask: &[u32; 4]) {
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_R, mask[0] as i32);
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_G, mask[1] as i32);
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_B, mask[2] as i32);
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_A, mask[3] as i32);
   }
}

pub fn to_vec2(vec: Vector) -> Vec2 {
   Vec2::new(vec.x, vec.y)
}

pub fn flip_vertically(width: usize, height: usize, channels: usize, data: &mut [u8]) {
   for y in 0..height / 2 {
      let inv_y = height - y - 1;
      for x in 0..width {
         for channel in 0..channels {
            let index_upper = (x + y * width) * 4 + channel;
            let index_lower = (x + inv_y * width) * 4 + channel;
            data.swap(index_upper, index_lower);
         }
      }
   }
}
