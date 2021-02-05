use std::ops::Deref;

use skulpin::skia_safe::*;

pub struct PaintCanvas<'a> {
    bitmap: Bitmap,
    canvas: OwnedCanvas<'a>,
}

impl PaintCanvas<'_> {

    pub fn new(size: (u32, u32)) -> Self {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels((size.0 as _, size.1 as _), None);
        let mut canvas = Canvas::from_bitmap(&bitmap, None);
        canvas.clear(Color::TRANSPARENT);
        Self {
            bitmap,
            canvas,
        }
    }

    pub fn stroke(&mut self, from: impl Into<Point>, to: impl Into<Point>) {
        let mut paint = Paint::new(Color4f::new(0.0, 0.0, 0.0, 1.0), None);
        paint.set_anti_alias(false);
        paint.set_style(paint::Style::Stroke);
        paint.set_stroke_width(4.0);
        paint.set_stroke_cap(paint::Cap::Round);
        self.canvas.draw_line(from.into(), to.into(), &paint);
    }

}

impl Deref for PaintCanvas<'_> {

    type Target = Bitmap;

    fn deref(&self) -> &Self::Target {
        &self.bitmap
    }

}
