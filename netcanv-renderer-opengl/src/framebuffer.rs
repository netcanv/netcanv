pub struct Framebuffer {}

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (0, 0)
   }

   fn upload_rgba(&mut self, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {}

   fn download_rgba(&self, dest: &mut [u8]) {}
}
