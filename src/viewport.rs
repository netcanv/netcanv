use skulpin::skia_safe::{Rect, IRect, Vector};

#[derive(Clone)]
pub struct Viewport {
    pan: Vector,
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
        }
    }

    pub fn pan(&self) -> Vector {
        self.pan
    }

    pub fn pan_around(&mut self, by: Vector) {
        self.pan.offset(by);
    }

    pub fn visible_rect(&self, window_size: (f32, f32)) -> Rect {
        Rect {
            left: self.pan.x,
            top: self.pan.y,
            right: self.pan.x + window_size.0,
            bottom: self.pan.y + window_size.1,
        }
    }

    pub fn visible_tiles(&self, tile_size: (i32, i32), window_size: (f32, f32)) -> Tiles<'_> {
        let visible_rect = self.visible_rect(window_size);
        let irect = IRect {
            left: (visible_rect.left / tile_size.0 as f32).floor() as i32,
            top: (visible_rect.top / tile_size.1 as f32).floor() as i32,
            right: (visible_rect.right / tile_size.0 as f32).ceil() as i32,
            bottom: (visible_rect.bottom / tile_size.1 as f32).ceil() as i32,
        };
        Tiles {
            viewport: self,
            rect: irect,
            x: irect.left,
            y: irect.top,
        }
    }

}

impl Iterator for Tiles<'_> {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        let pos = (self.x, self.y);

        self.x += 1;
        if self.x > self.rect.right {
            self.x = self.rect.left;
            self.y += 1;
        }
        if self.y > self.rect.bottom {
            None
        } else {
            Some(pos)
        }
    }
}
