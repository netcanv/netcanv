pub use paws;
use paws::{vector, Color, Point, Rect, Renderer, Vector};

/// A font.
pub trait Font {
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

/// Image and framebuffer scaling filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingFilter {
   /// Nearest neighbor. The default filter.
   Nearest,
   /// Bilinear.
   Linear,
}

impl Default for ScalingFilter {
   fn default() -> Self {
      Self::Nearest
   }
}

/// An image.
pub trait Image {
   /// _Colorizes_ an image by replacing all of its color with a single, solid color.
   ///
   /// The alpha channel in the resulting image is multiplied with the given color's alpha channel.
   ///
   /// # Implementation notes
   ///
   /// This operation must be cheap as it may be called multiple times per frame.
   fn colorized(&self, color: Color) -> Self;

   /// Returns the size of the image.
   fn size(&self) -> (u32, u32);

   /// Returns the width of the image.
   fn width(&self) -> u32 {
      self.size().0
   }

   /// Returns the height of the image.
   fn height(&self) -> u32 {
      self.size().1
   }

   /// Returns a rectangle sized as the image, with the provided position.
   fn rect(&self, position: Vector) -> Rect {
      Rect::new(position, vector(self.width() as f32, self.height() as f32))
   }
}

/// A framebuffer that can be rendered to.
pub trait Framebuffer {
   /// Returns the size of the framebuffer.
   fn size(&self) -> (u32, u32);

   /// Returns the width of the framebuffer.
   fn width(&self) -> u32 {
      self.size().0
   }

   /// Returns the height of the framebuffer.
   fn height(&self) -> u32 {
      self.size().1
   }

   /// Returns a rectangle sized as the framebuffer, with the provided position.
   fn rect(&self, position: Vector) -> Rect {
      Rect::new(position, vector(self.width() as f32, self.height() as f32))
   }

   /// Sets the filter used for upscaling and downscaling the framebuffer.
   fn set_scaling_filter(&mut self, filter: ScalingFilter);
}

/// Blending modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
   /// Does not blend colors.
   Replace,
   /// Blends colors using the alpha channel of the destination.
   Alpha,
   /// Adds colors together.
   Add,
   /// Inverts colors.
   Invert,
}

/// A render backend.
pub trait RenderBackend: Renderer {
   type Image: Image;
   type Framebuffer: Framebuffer;

   /// Creates a new image of the given size, from the given RGBA pixel data.
   fn create_image_from_rgba(&mut self, width: u32, height: u32, pixel_data: &[u8]) -> Self::Image;

   /// Creates a new font from the given in-memory TTF/OTF file, with a set default size.
   fn create_font_from_memory(&mut self, data: &[u8], default_size: f32) -> Self::Font;

   /// Creates a new framebuffer of the given size.
   ///
   /// The framebuffer should be cleared with transparent pixels.
   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer;

   /// Sets the current framebuffer to the provided one, calls `f`, and sets the framebuffer
   /// back to what it was before `draw_to` was called.
   fn draw_to(&mut self, framebuffer: &Self::Framebuffer, f: impl FnOnce(&mut Self));

   /// Clears the framebuffer with a solid color.
   fn clear(&mut self, color: Color);

   /// Draws an image such that it fills the given rectangle.
   fn image(&mut self, rect: Rect, image: &Self::Image);

   /// Draws a framebuffer such that it fills the given rectangle.
   ///
   /// Drawing the framebuffer that is currently being rendered to is undefined behavior.
   fn framebuffer(&mut self, rect: Rect, framebuffer: &Self::Framebuffer);

   /// Uploads RGBA pixels to the framebuffer.
   ///
   /// `pixels`'s length must be equal to `width * height * 4`.
   fn upload_framebuffer(
      &mut self,
      framebuffer: &Self::Framebuffer,
      position: (u32, u32),
      size: (u32, u32),
      pixels: &[u8],
   );

   /// Downloads RGBA pixels from the framebuffer into a buffer.
   fn download_framebuffer(
      &mut self,
      framebuffer: &Self::Framebuffer,
      position: (u32, u32),
      size: (u32, u32),
      out_pixels: &mut [u8],
   );

   /// Scales the transform matrix by the given factor.
   fn scale(&mut self, scale: Vector);

   /// Sets the current blend mode. Returns the old blend mode.
   ///
   /// Blend modes are part of the transformation stack. If used inside `push()` and `pop()`,
   /// the change is completely transparent to outside code.
   fn set_blend_mode(&mut self, new_blend_mode: BlendMode);

   /// Draws a filled circle, with the given center point, radius, and color.
   fn fill_circle(&mut self, center: Point, radius: f32, color: Color) {
      self.fill(
         Rect::new(
            center - vector(radius, radius),
            vector(radius, radius) * 2.0,
         ),
         color,
         radius,
      );
   }

   /// Draws an outlined circle, with the given center point, radius, and color.
   fn outline_circle(&mut self, center: Point, radius: f32, color: Color, thickness: f32) {
      self.outline(
         Rect::new(
            center - vector(radius, radius),
            vector(radius, radius) * 2.0,
         ),
         color,
         radius,
         thickness,
      );
   }
}
