use skulpin::skia_safe::{Rect, Vector};

#[derive(Clone)]
pub struct Viewport {
    pan: Vector,
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

    pub fn rect(&self, window_size: (f32, f32)) -> Rect {
        Rect {
            left: self.pan.x,
            top: self.pan.y,
            right: self.pan.x + window_size.0,
            bottom: self.pan.y + window_size.1,
        }
    }

}
