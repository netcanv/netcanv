use paws::{Color, Renderer};

/// A font.
pub trait Font {
   /// Creates a font from an in-memory file.
   fn from_memory(memory: &[u8], default_size: f32) -> Self;

   /// Creates a new font of the same typeface, but with a different size.
   ///
   /// Backends should optimize this operation to be as cheap as possible.
   fn with_size(&self, new_size: f32) -> Self;

   /// Returns the size of the font.
   ///
   /// **Note:** This is not the same as the font's height! This is the size that was passed in
   /// via the size parameter while the font was being created.
   fn size(&self) -> f32;
   /// Returns the height of the font, in pixels.
   fn height(&self) -> f32;

   /// Returns the width of the given text, when rendered with this font.
   fn text_width(&self, text: &str) -> f32;
}

/// An image.
pub trait Image {
   /// Creates an image from RGBA pixels.
   fn from_rgba(width: usize, height: usize, pixel_data: &[u8]) -> Self;

   /// Returns the size of the image.
   fn size(&self) -> (usize, usize);

   /// Returns the width of the image.
   fn width(&self) -> usize {
      self.size().0
   }

   /// Returns the height of the image.
   fn height(&self) -> usize {
      self.size().1
   }
}

/// A framebuffer that can be rendered to.
pub trait Framebuffer {
   /// Uploads RGBA pixels to the framebuffer.
   ///
   /// `pixels`'s length must be equal to `width * height * 4`.
   fn upload_rgba(&mut self, pixels: &[u8]);

   /// Downloads RGBA pixels from the framebuffer into a buffer.
   fn download_rgba(&self, dest: &mut [u8]);
}

/// A render backend.
pub trait RenderBackend: Renderer {
   type Framebuffer: Framebuffer;

   /// Creates a new framebuffer of the given size.
   ///
   /// The framebuffer should be cleared with transparent pixels.
   fn create_framebuffer(&self, width: usize, height: usize) -> Self::Framebuffer;

   /// Clears the framebuffer with a solid color.
   fn clear(&mut self, color: Color);
}
