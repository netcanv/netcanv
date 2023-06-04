use netcanv_renderer::paws::Color;

pub fn paws_color_to_wgpu(color: Color) -> wgpu::Color {
   wgpu::Color {
      r: color.r as f64 / 255.0,
      g: color.g as f64 / 255.0,
      b: color.b as f64 / 255.0,
      a: color.a as f64 / 255.0,
   }
}
