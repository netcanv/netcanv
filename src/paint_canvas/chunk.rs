use ::image::{ImageBuffer, Rgba, RgbaImage};
use netcanv_renderer::paws::Point;
use netcanv_renderer::{Framebuffer as FramebufferTrait, RenderBackend};

use crate::backend::{Backend, Framebuffer};

/// A chunk on the infinite canvas.
pub struct Chunk {
   pub framebuffer: Framebuffer,
   dirty: bool,
}

impl Chunk {
   /// The size of a sub-chunk.
   pub const SIZE: (u32, u32) = (256, 256);

   /// Creates a new chunk, using the given canvas as a Skia surface allocator.
   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         framebuffer: renderer.create_framebuffer(Self::SIZE.0, Self::SIZE.1),
         dirty: false,
      }
   }

   /// Returns the on-screen position of the chunk at the given coordinates.
   pub fn screen_position(chunk_position: (i32, i32)) -> Point {
      Point::new(
         (chunk_position.0 * Self::SIZE.0 as i32) as _,
         (chunk_position.1 * Self::SIZE.1 as i32) as _,
      )
   }

   /// Downloads the image of the chunk from the graphics card.
   pub fn download_image(&self) -> RgbaImage {
      let mut image_buffer =
         ImageBuffer::from_pixel(Self::SIZE.0, Self::SIZE.1, Rgba([0, 0, 0, 0]));
      self.framebuffer.download_rgba((0, 0), self.framebuffer.size(), &mut image_buffer);
      image_buffer
   }

   /// Uploads the image of the chunk to the graphics card, at the given offset in the master
   /// chunk.
   pub fn upload_image(&mut self, image: &RgbaImage, offset: (u32, u32)) {
      self.mark_dirty();
      self.framebuffer.upload_rgba(offset, Self::SIZE, image);
   }

   /// Marks the chunk as dirty - that is, invalidates any cached PNG and WebP data,
   /// and marks it as unsaved.
   pub fn mark_dirty(&mut self) {
      self.dirty = true;
   }

   /// Marks the given sub-chunk within this master chunk as saved.
   pub fn mark_saved(&mut self) {
      self.dirty = false;
   }

   /// Iterates through all pixels within the image and checks whether any pixels in the image are
   /// not transparent.
   pub fn image_is_empty(image: &RgbaImage) -> bool {
      image.iter().all(|x| *x == 0)
   }
}
