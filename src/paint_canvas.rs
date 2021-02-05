use std::ops::Deref;

use skulpin::skia_safe::*;

#[derive(Clone)]
pub enum Brush {
    Draw { color: Color4f, stroke_width: f32 },
    Erase { stroke_width: f32 },
}

impl Brush {

    pub fn as_paint(&self) -> Paint {
        let mut paint = Paint::new(Color4f::from(Color::TRANSPARENT), None);
        paint.set_anti_alias(false);
        paint.set_style(paint::Style::Stroke);
        paint.set_stroke_cap(paint::Cap::Round);

        match self {
            Self::Draw { color, stroke_width } => {
                paint.set_color(color.to_color());
                paint.set_stroke_width(*stroke_width);
            },
            Self::Erase { stroke_width } => {
                paint.set_blend_mode(BlendMode::Clear);
                paint.set_stroke_width(*stroke_width);
            },
        }

        paint
    }
}

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

    pub fn stroke(
        &mut self,
        from: impl Into<Point>,
        to: impl Into<Point>,
        brush: &Brush
    ) {
        let mut paint = brush.as_paint();
        self.canvas.draw_line(from.into(), to.into(), &paint);
    }

}

impl Deref for PaintCanvas<'_> {

    type Target = Bitmap;

    fn deref(&self) -> &Self::Target {
        &self.bitmap
    }

}
