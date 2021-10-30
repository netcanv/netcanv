//! A <del>quite shitty</del> text renderer based on FreeType.

pub struct Font {}

impl netcanv_renderer::Font for Font {
   fn from_memory(memory: &[u8], default_size: f32) -> Self {
      Self {}
   }

   fn with_size(&self, new_size: f32) -> Self {
      Self {}
   }

   fn size(&self) -> f32 {
      14.0
   }

   fn height(&self) -> f32 {
      14.0
   }

   fn text_width(&self, text: &str) -> f32 {
      32.0
   }
}
