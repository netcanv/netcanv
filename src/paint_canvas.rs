use std::collections::HashMap;
use std::collections::HashSet;

use skulpin::skia_safe::*;

#[derive(Clone, Debug)]
pub enum Brush {
    Draw { color: Color4f, stroke_width: f32 },
    Erase { stroke_width: f32 },
}

#[derive(Debug)]
pub struct StrokePoint {
    pub point: Point,
    pub brush: Brush,
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

const CHUNK_SIZE: (i32, i32) = (256, 256);

struct Chunk<'a> {
    bitmap: Bitmap,
    canvas: OwnedCanvas<'a>,
}

impl Chunk<'_> {

    fn new() -> Self {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels(CHUNK_SIZE, None);
        let mut canvas = Canvas::from_bitmap(&bitmap, None);
        canvas.clear(Color::TRANSPARENT);
        Self {
            bitmap,
            canvas,
        }
    }

    fn screen_position(chunk_position: (i32, i32)) -> Point {
        Point::new(
            (chunk_position.0 * CHUNK_SIZE.0) as _,
            (chunk_position.1 * CHUNK_SIZE.1) as _,
        )
    }

}

pub struct PaintCanvas<'a> {
    chunks: HashMap<(i32, i32), Chunk<'a>>,
    // this set contains all chunks that have already been visited in the current stroke() call
    stroked_chunks: HashSet<(i32, i32)>,
}

impl<'a> PaintCanvas<'a> {

    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            stroked_chunks: HashSet::new(),
        }
    }

    fn ensure_chunk_exists(&mut self, position: (i32, i32)) {
        if !self.chunks.contains_key(&position) {
            self.chunks.insert(position, Chunk::new());
        }
    }

    pub fn stroke(
        &mut self,
        from: impl Into<Point>,
        to: impl Into<Point>,
        brush: &Brush,
    ) {
        let a = from.into();
        let b = to.into();
        let step_count = i32::max((Point::distance(a, b) / 4.0) as _, 2);
        let paint = brush.as_paint();
        let stroke_width = paint.stroke_width();
        let half_stroke_width = stroke_width / 2.0;

        let mut delta = b - a;
        delta.x /= step_count as f32;
        delta.y /= step_count as f32;
        let mut p = a;

        self.stroked_chunks.clear();
        for _ in 1..=step_count {
            let top_left = p - Point::new(half_stroke_width, half_stroke_width);
            let bottom_right = p + Point::new(half_stroke_width, half_stroke_width);
            let top_left_chunk = (
                (top_left.x / CHUNK_SIZE.0 as f32).floor() as i32,
                (top_left.y / CHUNK_SIZE.0 as f32).floor() as i32,
            );
            let bottom_right_chunk = (
                (bottom_right.x / CHUNK_SIZE.1 as f32).ceil() as i32,
                (bottom_right.y / CHUNK_SIZE.1 as f32).ceil() as i32,
            );

            for y in top_left_chunk.1 .. bottom_right_chunk.1 {
                for x in top_left_chunk.0 .. bottom_right_chunk.0 {
                    let chunk_position = (x, y);
                    if !self.stroked_chunks.contains(&chunk_position) {
                        self.ensure_chunk_exists(chunk_position);
                        let chunk = self.chunks.get_mut(&chunk_position).unwrap();
                        let screen_position = Chunk::screen_position(chunk_position);
                        chunk.canvas.draw_line(a - screen_position, b - screen_position, &paint);
                    }
                    self.stroked_chunks.insert(chunk_position);
                    p.offset(delta);
                }
            }
        }

    }

    pub fn draw_to(
        &self,
        canvas: &mut Canvas,
    ) {
        for (chunk_position, chunk) in &self.chunks {
            let screen_position = Chunk::screen_position(*chunk_position);
            canvas.draw_bitmap(&chunk.bitmap, screen_position, None);
        }
    }

}

