use netcanv_renderer::paws::Color;

pub fn normalized_color(color: Color) -> (f32, f32, f32, f32) {
   (
      color.r as f32 / 255.0,
      color.g as f32 / 255.0,
      color.b as f32 / 255.0,
      color.a as f32 / 255.0,
   )
}
