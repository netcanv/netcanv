//! A really simple horizontal shelf rectangle packer used by the font renderer.

use netcanv_renderer::paws::{point, vector, Rect};

const PADDING: f32 = 1.0;

pub struct RectPacker {
   width: f32,
   height: f32,
   shelf_x: f32,
   shelf_y: f32,
   shelf_height: f32,
}

impl RectPacker {
   pub fn new(width: f32, height: f32) -> Self {
      let mut packer = Self {
         width,
         height,
         shelf_x: 0.0,
         shelf_y: 0.0,
         shelf_height: 0.0,
      };
      // Pack a single pixel
      packer.pack(1.0, 1.0);
      packer
   }

   pub fn pack(&mut self, width: f32, height: f32) -> Option<Rect> {
      if width == 0.0 && height == 0.0 {
         // Special case: return the first pixel packed.
         return Some(Rect::new(point(0.0, 0.0), vector(0.0, 0.0)));
      }
      if self.shelf_x + width + PADDING >= self.width {
         // Wrap around to the next shelf.
         self.shelf_x = 0.0;
         self.shelf_y += self.shelf_height;
         self.shelf_height = 0.0;
      }
      if self.shelf_y + height + PADDING >= self.height {
         // No vertical space left.
         return None;
      }
      let position = point(self.shelf_x + PADDING, self.shelf_y + PADDING);
      self.shelf_x += width + PADDING;
      self.shelf_height = f32::max(self.shelf_height, height + PADDING);
      Some(Rect::new(position, vector(width, height)))
   }
}
