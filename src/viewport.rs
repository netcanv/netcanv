//! Panning and zooming.

use skulpin::skia_safe::{IRect, Point, Rect, Vector};

/// A viewport that can be panned around and zoomed into.
#[derive(Clone)]
pub struct Viewport {
    pan: Vector,
    zoom_level: f32,
}

/// An iterator over tiles visible in a viewport.
pub struct Tiles {
    rect: IRect,
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
        self.pan.offset(by * (1.0 / self.zoom()));
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
    pub fn visible_rect(&self, window_size: (f32, f32)) -> Rect {
        let inv_zoom = 1.0 / self.zoom();
        let half_width = window_size.0 * inv_zoom / 2.0;
        let half_height = window_size.1 * inv_zoom / 2.0;
        Rect {
            left: self.pan.x - half_width,
            top: self.pan.y - half_height,
            right: self.pan.x + half_width,
            bottom: self.pan.y + half_height,
        }
    }

    /// Returns an iterator over equally-sized square tiles seen from the viewport.
    pub fn visible_tiles(&self, tile_size: (u32, u32), window_size: (f32, f32)) -> Tiles {
        let visible_rect = self.visible_rect(window_size);
        let irect = IRect {
            left: (visible_rect.left / tile_size.0 as f32).floor() as i32,
            top: (visible_rect.top / tile_size.1 as f32).floor() as i32,
            right: (visible_rect.right / tile_size.0 as f32).floor() as i32,
            bottom: (visible_rect.bottom / tile_size.1 as f32).floor() as i32,
        };
        Tiles {
            rect: irect,
            x: irect.left,
            y: irect.top,
        }
    }

    /// Converts a point from screen space to viewport space.
    ///
    /// This can be used to pick things on the canvas, given a mouse position.
    pub fn to_viewport_space(&self, point: impl Into<Point>, window_size: (f32, f32)) -> Point {
        (point.into() - Point::from(window_size) * 0.5) * (1.0 / self.zoom()) + self.pan
    }

    /// Converts a point from viewport space to screen space.
    ///
    /// This transformation is the inverse of [`Viewport::to_viewport_space`].
    pub fn to_screen_space(&self, point: impl Into<Point>, window_size: (f32, f32)) -> Point {
        (point.into() - self.pan) * self.zoom() + Point::from(window_size) * 0.5
    }
}

impl Iterator for Tiles {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        let pos = (self.x, self.y);

        self.x += 1;
        if self.y > self.rect.bottom {
            return None
        }
        if self.x > self.rect.right {
            self.x = self.rect.left;
            self.y += 1;
        }
        Some(pos)
    }
}
