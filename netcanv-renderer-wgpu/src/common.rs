use glam::{vec2, Vec2};
use netcanv_renderer::paws::{Color, Vector};

pub fn vector_to_vec2(vector: Vector) -> Vec2 {
   vec2(vector.x, vector.y)
}

pub fn paws_color_to_wgpu(color: Color) -> wgpu::Color {
   wgpu::Color {
      r: color.r as f64 / 255.0,
      g: color.g as f64 / 255.0,
      b: color.b as f64 / 255.0,
      a: color.a as f64 / 255.0,
   }
}
