use netcanv_renderer::paws::Color;

pub struct Image {}

impl netcanv_renderer::Image for Image {
   fn from_rgba(width: u32, height: u32, pixel_data: &[u8]) -> Self {
      Self {}
   }

   fn colorized(&self, color: Color) -> Self {
      Self {}
   }

   fn size(&self) -> (usize, usize) {
      (0, 0)
   }
}
