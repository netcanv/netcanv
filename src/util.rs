use std::cell::RefCell;
use std::rc::Rc;

use skulpin::skia_safe::*;
use skulpin::CoordinateSystemHelper;

// colors

pub fn hex_color4f(hex: u32) -> Color4f {
    let r = ((hex >> 24) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let b = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let a = (hex & 0xFF) as f32 / 255.0;
    Color4f::new(r, g, b, a)
}

// conversions

pub fn get_window_size(coordinate_system_helper: &CoordinateSystemHelper) -> (f32, f32) {
    let logical_size = coordinate_system_helper.window_logical_size();
    (logical_size.width as _, logical_size.height as _)
}

// resources

pub type RcFont = Rc<RefCell<Font>>;

pub fn new_rc_font(data: &[u8], default_size: f32) -> RcFont {
    let typeface = Typeface::from_data(Data::new_copy(data), None).expect("failed to load font");
    Rc::new(RefCell::new(Font::new(typeface, default_size)))
}

// math

pub fn quantize(value: f32, step: f32) -> f32 {
    step * (value / step + 0.5).floor()
}
