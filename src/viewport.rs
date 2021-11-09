//! Panning and zooming.

use netcanv_renderer::paws::{point, vector, Point, Rect, Vector};

/// A viewport that can be panned around and zoomed into.
#[derive(Clone)]
pub struct Viewport {
   pan: Vector,
   zoom_level: f32,
}

/// A rectangle with integer coordinates.
pub struct IntRect {
   right: i32,
   bottom: i32,
   left: i32,
   top: i32,
}

/// An iterator over tiles visible in a viewport.
pub struct Tiles {
   rect: IntRect,
   x: i32,
   y: i32,
}

impl Viewport {
   /// Creates a new viewport.
   pub fn new() -> Self {
      Self {
         pan: Vector::new(0.0, 0.0),
         zoom_level: 0.0,
      }
   }

   /// Returns the panning vector.
   pub fn pan(&self) -> Vector {
      self.pan
   }

   /// Returns the zoom factor.
   pub fn zoom(&self) -> f32 {
      f32::powf(2.0, self.zoom_level * 0.25)
   }

   /// Pans the viewport around by the given vector.
   pub fn pan_around(&mut self, by: Vector) {
      self.pan += by * (1.0 / self.zoom());
   }

   /// Zooms in or out of the viewport by the given delta.
   ///
   /// Note that the delta does not influence the zoom factor directly. It instead modifies the
   /// _zoom level_, which is linear, and this zoom level is later converted into the
   /// exponential _zoom factor_.
   pub fn zoom_in(&mut self, delta: f32) {
      self.zoom_level += delta;
      self.zoom_level = self.zoom_level.clamp(-16.0, 24.0);
   }

   /// Returns the rectangle visible from the viewport, given the provided window size.
   pub fn visible_rect(&self, window_size: Vector) -> Rect {
      let inv_zoom = 1.0 / self.zoom();
      let width = window_size.x * inv_zoom;
      let height = window_size.y * inv_zoom;
      Rect::new(
         point(self.pan.x - width / 2.0, self.pan.y - height / 2.0),
         vector(width, height),
      )
   }

   /// Returns an iterator over equally-sized square tiles seen from the viewport.
   pub fn visible_tiles(&self, tile_size: (u32, u32), window_size: Vector) -> Tiles {
      let visible_rect = self.visible_rect(window_size);
      let irect = IntRect {
         left: (visible_rect.left() / tile_size.0 as f32).floor() as i32,
         top: (visible_rect.top() / tile_size.1 as f32).floor() as i32,
         right: (visible_rect.right() / tile_size.0 as f32).floor() as i32,
         bottom: (visible_rect.bottom() / tile_size.1 as f32).floor() as i32,
      };
      let x = irect.left;
      let y = irect.top;
      Tiles { rect: irect, x, y }
   }

   /// Converts a point from screen space to viewport space.
   ///
   /// This can be used to pick things on the canvas, given a mouse position.
   pub fn to_viewport_space(&self, point: Point, window_size: Vector) -> Point {
      (point - window_size / 2.0) * (1.0 / self.zoom()) + self.pan
   }

   /// Converts a point from viewport space to screen space.
   ///
   /// This transformation is the inverse of [`Viewport::to_viewport_space`].
   pub fn to_screen_space(&self, point: Point, window_size: Vector) -> Point {
      (point - self.pan) * self.zoom() + window_size / 2.0
   }
}

impl Iterator for Tiles {
   type Item = (i32, i32);

   fn next(&mut self) -> Option<Self::Item> {
      let pos = (self.x, self.y);

      self.x += 1;
      if self.y > self.rect.bottom {
         return None;
      }
      if self.x > self.rect.right {
         self.x = self.rect.left;
         self.y += 1;
      }
      Some(pos)
   }
}
