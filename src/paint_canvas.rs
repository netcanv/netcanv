use std::collections::{HashMap, HashSet, hash_map};
use std::io::Cursor;
use std::ops::{Deref, DerefMut};
use std::path::Path;

use skulpin::skia_safe::*;
use ::image::{
    ColorType,
    GenericImage,
    GenericImageView,
    ImageDecoder,
    ImageBuffer,
    ImageError,
    Rgba,
    RgbaImage,
    codecs::png::{PngDecoder, PngEncoder}
};

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


pub struct Chunk<'a> {
    bitmap: Bitmap,
    canvas: OwnedCanvas<'a>,
    png_data: Option<Vec<u8>>,
}

impl<'a> Chunk<'a> {
    const SIZE: (i32, i32) = (256, 256);


    fn new() -> Self {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels(Self::SIZE, None);
        let mut canvas = Canvas::from_bitmap(&bitmap, None);
        canvas.clear(Color::TRANSPARENT);
        Self {
            bitmap,
            canvas,
            png_data: None,
        }
    }

    fn screen_position(chunk_position: (i32, i32)) -> Point {
        Point::new(
            (chunk_position.0 * Self::SIZE.0) as _,
            (chunk_position.1 * Self::SIZE.1) as _,
        )
    }

    fn pixels_mut(&mut self) -> &'a mut [u8] {
        unsafe {
            // I REALLY HOPE THIS IS CORRECT :)
            let rawptr = self.bitmap.pixels() as *mut u8;
            std::slice::from_raw_parts_mut(rawptr, self.bitmap.compute_byte_size())
        }
    }

    fn pixels(&self) -> &'a [u8] {
        unsafe {
            // i hope *even more* that _this_ is correct, as sus at it looks everything should be fine.
            // ~ top 10 quotes said before tragedy
            let bitmap = &self.bitmap as *const Bitmap as *mut Bitmap;
            let rawptr = (*bitmap).pixels() as *const u8;
            std::slice::from_raw_parts(rawptr, self.bitmap.compute_byte_size())
        }
    }

    fn as_image_buffer(&self) -> ImageBuffer<Rgba<u8>, &'a [u8]> {
        ImageBuffer::from_raw(Self::SIZE.0 as u32, Self::SIZE.1 as u32, self.pixels()).unwrap()
    }

    fn as_image_buffer_mut(&mut self) -> ImageBuffer<Rgba<u8>, &'a mut [u8]> {
        ImageBuffer::from_raw(Self::SIZE.0 as u32, Self::SIZE.1 as u32, self.pixels_mut()).unwrap()
    }

    // reencodes PNG data if necessary.
    // PNG data is reencoded upon outside request, but invalidated if the chunk is modified
    fn png_data(&mut self) -> Option<&[u8]> {
        if self.png_data.is_none() {
            let pixels = self.pixels_mut();
            let (width, height) = (self.bitmap.width() as u32, self.bitmap.height() as u32);
            let mut bytes: Vec<u8> = Vec::new();
            if PngEncoder::new(Cursor::new(&mut bytes)).encode(pixels, width, height, ColorType::Rgba8).is_err() {
                return None
            }
            self.png_data = Some(bytes);
        }
        Some(self.png_data.as_ref().unwrap())
    }

    fn decode_png_data(&mut self, data: &[u8]) -> Result<(), ImageError> {
        let decoder = PngDecoder::new(Cursor::new(data))?;
        if decoder.color_type() != ColorType::Rgba8 {
            eprintln!("received non-RGBA image data, ignoring");
            return Ok(())
        }
        if decoder.dimensions() != (Self::SIZE.0 as u32, Self::SIZE.1 as u32) {
            eprintln!("received chunk with invalid size. got: {:?}, expected: {:?}", decoder.dimensions(), Self::SIZE);
            return Ok(())
        }
        decoder.read_image(self.pixels_mut())?;
        Ok(())
    }

}

pub struct PaintCanvas<'a> {
    chunks: HashMap<(i32, i32), Chunk<'a>>,
    // this set contains all chunks that have already been visited in the current stroke() call
    stroked_chunks: HashSet<(i32, i32)>,
}

pub struct PngData<'a, 'b> {
    iter: hash_map::IterMut<'a, (i32, i32), Chunk<'b>>,
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
                (top_left.x / Chunk::SIZE.0 as f32).floor() as i32,
                (top_left.y / Chunk::SIZE.0 as f32).floor() as i32,
            );
            let bottom_right_chunk = (
                (bottom_right.x / Chunk::SIZE.1 as f32).ceil() as i32,
                (bottom_right.y / Chunk::SIZE.1 as f32).ceil() as i32,
            );

            for y in top_left_chunk.1 .. bottom_right_chunk.1 {
                for x in top_left_chunk.0 .. bottom_right_chunk.0 {
                    let chunk_position = (x, y);
                    if !self.stroked_chunks.contains(&chunk_position) {
                        self.ensure_chunk_exists(chunk_position);
                        let chunk = self.chunks.get_mut(&chunk_position).unwrap();
                        let screen_position = Chunk::screen_position(chunk_position);
                        chunk.canvas.draw_line(a - screen_position, b - screen_position, &paint);
                        chunk.png_data = None;
                    }
                    self.stroked_chunks.insert(chunk_position);
                    p.offset(delta);
                }
            }
        }

    }

    pub fn draw_to(&self, canvas: &mut Canvas) {
        for (chunk_position, chunk) in &self.chunks {
            let screen_position = Chunk::screen_position(*chunk_position);
            canvas.draw_bitmap(&chunk.bitmap, screen_position, None);
        }
    }

    pub fn png_data(&mut self) -> PngData<'_, 'a> {
        PngData {
            iter: self.chunks.iter_mut(),
        }
    }

    pub fn decode_png_data(&mut self, to_chunk: (i32, i32), data: &[u8]) -> Result<(), ImageError> {
        self.ensure_chunk_exists(to_chunk);
        let chunk = self.chunks.get_mut(&to_chunk).unwrap();
        chunk.decode_png_data(data)
    }

    pub fn cleanup_empty_chunks(&mut self) {
        self.chunks.retain(|_, chunk| {
            chunk.pixels().iter().any(|x| *x != 0u8)
        });
    }

    // right now loading/saving only really works (well, was tested) on little-endian machines, so i make no guarantees
    // if it works on big-endian. most likely loading will screw up the channel order in pixels. thanks, skia!

    fn fix_endianness<C>(image: &mut ImageBuffer<Rgba<u8>, C>)
        where C: Deref<Target = [u8]> + DerefMut
    {
        #[cfg(target_endian = "little")]
        {
            use ::image::Pixel;
            for pixel in image.pixels_mut() {
                let bgra = pixel.to_bgra();
                let channels = pixel.channels_mut();
                channels[0] = bgra[0];
                channels[1] = bgra[1];
                channels[2] = bgra[2];
                channels[3] = bgra[3];
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), anyhow::Error> {
        let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for (chunk_position, _) in &self.chunks {
            left = left.min(chunk_position.0);
            top = top.min(chunk_position.1);
            right = right.max(chunk_position.0);
            bottom = bottom.max(chunk_position.1);
        }
        eprintln!("left={}, top={}, right={}, bottom={}", left, top, right, bottom);
        if left == i32::MAX {
            anyhow::bail!("There's nothing to save! Draw something on the canvas and try again.");
        }
        let width = ((right - left + 1) * Chunk::SIZE.0) as u32;
        let height = ((bottom - top + 1) * Chunk::SIZE.1) as u32;
        eprintln!("size: {:?}", (width, height));
        let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        for (chunk_position, chunk) in &self.chunks {
            eprintln!("writing chunk {:?}", chunk_position);
            let pixel_position = (
                (Chunk::SIZE.0 * (chunk_position.0 - left)) as u32,
                (Chunk::SIZE.1 * (chunk_position.1 - top)) as u32,
            );
            eprintln!("   - pixel position: {:?}", pixel_position);
            let pixels = Vec::from(chunk.pixels());
            let mut chunk_image = RgbaImage::from_vec(Chunk::SIZE.0 as u32, Chunk::SIZE.1 as u32, pixels).unwrap();
            Self::fix_endianness(&mut chunk_image);
            let mut sub_image = image.sub_image(
                pixel_position.0,
                pixel_position.1,
                Chunk::SIZE.0 as u32,
                Chunk::SIZE.1 as u32,
            );
            sub_image.copy_from(&chunk_image, 0, 0)?;
        }
        image.save(path)?;
        Ok(())
    }

    pub fn load_from_image_file(&mut self, path: &Path) -> Result<(), anyhow::Error> {
        use ::image::io::Reader as ImageReader;

        let image = ImageReader::open(path)?.decode()?.into_rgba8();
        eprintln!("image size: {:?}", image.dimensions());
        let chunks_x = (image.width() as f32 / Chunk::SIZE.0 as f32).ceil() as i32;
        let chunks_y = (image.height() as f32 / Chunk::SIZE.1 as f32).ceil() as i32;
        eprintln!("n. chunks: x={}, y={}", chunks_x, chunks_y);

        for y in 0..chunks_y {
            for x in 0..chunks_x {
                let chunk_position = (x, y);
                self.ensure_chunk_exists(chunk_position);
                let chunk = self.chunks.get_mut(&chunk_position).unwrap();
                let mut chunk_image = chunk.as_image_buffer_mut();
                let pixel_position = (
                    (Chunk::SIZE.0 * chunk_position.0) as u32,
                    (Chunk::SIZE.1 * chunk_position.1) as u32,
                );
                eprintln!("plopping chunk at {:?}", pixel_position);
                let right = (pixel_position.0 + Chunk::SIZE.0 as u32).min(image.width() - 1);
                let bottom = (pixel_position.1 + Chunk::SIZE.1 as u32).min(image.height() - 1);
                eprintln!("  to {:?}", (right, bottom));
                let width = right - pixel_position.0;
                let height = bottom - pixel_position.1;
                let sub_image = image.view(pixel_position.0, pixel_position.1, width, height);
                chunk_image.copy_from(&sub_image, 0, 0)?;
                Self::fix_endianness(&mut chunk_image);
            }
        }

        Ok(())
    }

}

impl Iterator for PngData<'_, '_> {
    type Item = ((i32, i32), Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((position, chunk)) = self.iter.next() {
            if let Some(png_data) = chunk.png_data() {
                return Some((*position, Vec::from(png_data)))
            }
        }
        None
    }
}
