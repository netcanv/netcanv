use skulpin::skia_safe::{IRect, Point, Rect, Vector};

#[derive(Clone)]
pub struct Viewport {
    pan: Vector,
    zoom_level: f32,
}

pub struct Tiles<'a> {
    viewport: &'a Viewport,
    rect: IRect,
    x: i32,
    y: i32,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            pan: Vector::new(0.0, 0.0),
            zoom_level: 0.0,
        }
    }

    pub fn pan(&self) -> Vector {
        self.pan
    }

    pub fn zoom(&self) -> f32 {
        f32::powf(2.0, self.zoom_level * 0.25)
    }

    pub fn pan_around(&mut self, by: Vector) {
        self.pan.offset(by * (1.0 / self.zoom()));
    }

    pub fn zoom_in(&mut self, delta: f32) {
        self.zoom_level += delta;
        self.zoom_level = self.zoom_level.clamp(-14.0, 24.0);
    }

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

    pub fn visible_tiles(&self, tile_size: (u32, u32), window_size: (f32, f32)) -> Tiles<'_> {
        let visible_rect = self.visible_rect(window_size);
        let irect = IRect {
            left: (visible_rect.left / tile_size.0 as f32).floor() as i32,
            top: (visible_rect.top / tile_size.1 as f32).floor() as i32,
            right: (visible_rect.right / tile_size.0 as f32).floor() as i32,
            bottom: (visible_rect.bottom / tile_size.1 as f32).floor() as i32,
        };
        Tiles {
            viewport: self,
            rect: irect,
            x: irect.left,
            y: irect.top,
        }
    }

    pub fn to_viewport_space(&self, point: impl Into<Point>, window_size: (f32, f32)) -> Point {
        // (point.into() - Point::from(window_size) * 0.5 + self.pan * self.zoom()) * (1.0 / self.zoom())
        (point.into() - Point::from(window_size) * 0.5) * (1.0 / self.zoom()) + self.pan
    }

    pub fn to_screen_space(&self, point: impl Into<Point>, window_size: (f32, f32)) -> Point {
        (point.into() - self.pan) * self.zoom() + Point::from(window_size) * 0.5
    }
}

impl Iterator for Tiles<'_> {
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
