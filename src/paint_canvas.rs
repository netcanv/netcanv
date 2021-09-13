use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use ::image::codecs::png::{PngDecoder, PngEncoder};
use ::image::{ColorType, DynamicImage, GenericImage, GenericImageView, ImageBuffer, ImageDecoder, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use skulpin::skia_safe as skia;
use skulpin::skia_safe::*;

use crate::viewport::Viewport;

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

pub struct Chunk {
    surface: RefCell<Surface>,
    png_data: [Option<Vec<u8>>; Self::SUB_COUNT],
    webp_data: [Option<Vec<u8>>; Self::SUB_COUNT],
    non_empty_subs: [bool; Self::SUB_COUNT],
    saved_subs: [bool; Self::SUB_COUNT],
}

impl Chunk {
    const MAX_PNG_SIZE: usize = 32 * 1024;
    pub const SIZE: (u32, u32) = (256, 256);
    const SUB_CHUNKS: (u32, u32) = (4, 4);
    const SUB_COUNT: usize = (Self::SUB_CHUNKS.0 * Self::SUB_CHUNKS.1) as usize;
    const SURFACE_SIZE: (u32, u32) = (
        (Self::SIZE.0 * Self::SUB_CHUNKS.0) as u32,
        (Self::SIZE.1 * Self::SUB_CHUNKS.1) as u32,
    );
    const WEBP_QUALITY: f32 = 0.9;

    fn new(canvas: &mut Canvas) -> Self {
        let surface = match canvas.new_surface(&Self::image_info(Self::SURFACE_SIZE), None) {
            Some(surface) => surface,
            None => panic!("failed to create a surface for storing the chunk"),
        };
        Self {
            surface: RefCell::new(surface),
            png_data: Default::default(),
            webp_data: Default::default(),
            non_empty_subs: [false; Self::SUB_COUNT],
            saved_subs: [false; Self::SUB_COUNT],
        }
    }

    fn screen_position(chunk_position: (i32, i32)) -> Point {
        Point::new(
            (chunk_position.0 * Self::SURFACE_SIZE.0 as i32) as _,
            (chunk_position.1 * Self::SURFACE_SIZE.1 as i32) as _,
        )
    }

    fn download_image(&self) -> RgbaImage {
        let mut image_buffer = ImageBuffer::from_pixel(Self::SURFACE_SIZE.0, Self::SURFACE_SIZE.1, Rgba([0, 0, 0, 0]));
        self.surface.borrow_mut().read_pixels(
            &Self::image_info(Self::SURFACE_SIZE),
            &mut image_buffer,
            Self::SURFACE_SIZE.0 as usize * 4,
            (0, 0),
        );
        image_buffer
    }

    fn upload_image(&mut self, image: RgbaImage, offset: (u32, u32)) {
        let pixmap = Pixmap::new(
            &Self::image_info(image.dimensions()),
            &image,
            image.width() as usize * 4,
        );
        self.surface
            .borrow_mut()
            .write_pixels_from_pixmap(&pixmap, (offset.0 as i32, offset.1 as i32));
    }

    // get master chunk position from absolute position
    fn master(chunk_position: (i32, i32)) -> (i32, i32) {
        (
            chunk_position.0.div_euclid(Self::SUB_CHUNKS.0 as i32),
            chunk_position.1.div_euclid(Self::SUB_CHUNKS.1 as i32),
        )
    }

    // get sub chunk position from absolute position
    fn sub(chunk_position: (i32, i32)) -> usize {
        let x_bits = chunk_position.0.rem_euclid(Self::SUB_CHUNKS.0 as i32) as usize;
        let y_bits = chunk_position.1.rem_euclid(Self::SUB_CHUNKS.1 as i32) as usize;
        (x_bits << 2) | y_bits
    }

    // position of the given sub in a master chunk
    fn sub_position(sub: usize) -> (u32, u32) {
        (((sub & 0b1100) >> 2) as u32, (sub & 0b11) as u32)
    }

    // on-image position of the given sub in a master chunk
    fn sub_screen_position(sub: usize) -> (u32, u32) {
        (
            ((sub & 0b1100) >> 2) as u32 * Self::SIZE.0,
            (sub & 0b11) as u32 * Self::SIZE.1,
        )
    }

    // network position of a sub chunk in a master chunk
    fn chunk_position(master_position: (i32, i32), sub: usize) -> (i32, i32) {
        let sub_position = Self::sub_position(sub);
        (
            (master_position.0 * Self::SUB_CHUNKS.0 as i32) + sub_position.0 as i32,
            (master_position.1 * Self::SUB_CHUNKS.1 as i32) + sub_position.1 as i32,
        )
    }

    // reencodes PNG data if necessary.
    // PNG data is reencoded upon outside request, but invalidated if the chunk is modified
    fn png_data(&mut self, sub: usize) -> Option<&[u8]> {
        if self.png_data[sub].is_none() {
            eprintln!("  png data doesn't exist, encoding");
            let chunk_image = self.download_image();
            for sub in 0..Self::SUB_COUNT {
                let (x, y) = Self::sub_screen_position(sub);
                let sub_image = chunk_image.view(x, y, Self::SIZE.0, Self::SIZE.1).to_image();
                if Self::image_is_empty(&sub_image) {
                    self.non_empty_subs[sub] = false;
                    continue
                }
                let mut bytes: Vec<u8> = Vec::new();
                match PngEncoder::new(Cursor::new(&mut bytes)).encode(
                    &sub_image,
                    sub_image.width(),
                    sub_image.height(),
                    ColorType::Rgba8,
                ) {
                    Ok(()) => (),
                    Err(error) => {
                        eprintln!("error while encoding: {}", error);
                        continue
                    },
                }
                self.png_data[sub] = Some(bytes);
                self.non_empty_subs[sub] = true;
            }
        }
        self.png_data[sub].as_deref()
    }

    fn webp_data(&mut self, sub: usize) -> Option<&[u8]> {
        if self.webp_data[sub].is_none() {
            eprintln!("  webp data doesn't exist, encoding");
            let chunk_image = self.download_image();
            for sub in 0..Self::SUB_COUNT {
                let (x, y) = Self::sub_screen_position(sub);
                let sub_image = chunk_image.view(x, y, Self::SIZE.0, Self::SIZE.1).to_image();
                if Self::image_is_empty(&sub_image) {
                    self.non_empty_subs[sub] = false;
                    continue
                }
                let image = DynamicImage::ImageRgba8(sub_image);
                let encoder = webp::Encoder::from_image(&image).unwrap();
                let bytes = encoder.encode(Self::WEBP_QUALITY);
                self.webp_data[sub] = Some(bytes.to_owned());
                self.non_empty_subs[sub] = true;
            }
        }
        self.webp_data[sub].as_deref()
    }

    fn network_data(&mut self, sub: usize) -> Option<&[u8]> {
        let png_data = self.png_data(sub)?;
        if png_data.len() > Self::MAX_PNG_SIZE {
            eprintln!(
                "  png data is larger than {} KiB, fetching webp data instead",
                Self::MAX_PNG_SIZE / 1024
            );
            self.webp_data(sub)
        } else {
            // Need to call the function here a second time because otherwise the borrow checker
            // gets mad.
            self.png_data(sub)
        }
    }

    fn decode_png_data(&mut self, sub: usize, data: &[u8]) -> anyhow::Result<()> {
        let decoder = PngDecoder::new(Cursor::new(data))?;
        if decoder.color_type() != ColorType::Rgba8 {
            eprintln!("received non-RGBA image data, ignoring");
            return Ok(())
        }
        if decoder.dimensions() != Self::SIZE {
            eprintln!(
                "received chunk with invalid size. got: {:?}, expected: {:?}",
                decoder.dimensions(),
                Self::SIZE
            );
            return Ok(())
        }
        let mut image = RgbaImage::from_pixel(Self::SIZE.0, Self::SIZE.1, Rgba([0, 0, 0, 0]));
        decoder.read_image(&mut image)?;
        if !Self::image_is_empty(&image) {
            self.upload_image(image, Self::sub_screen_position(sub));
            self.png_data[sub] = Some(Vec::from(data));
            self.non_empty_subs[sub] = true;
        }
        Ok(())
    }

    fn decode_webp_data(&mut self, sub: usize, data: &[u8]) -> anyhow::Result<()> {
        let image = match webp::Decoder::new(data).decode() {
            Some(image) => image.to_image(),
            None => anyhow::bail!("got non-webp image"),
        }
        .into_rgba8();
        if !Self::image_is_empty(&image) {
            self.upload_image(image, Self::sub_screen_position(sub));
            self.webp_data[sub] = Some(Vec::from(data));
            self.non_empty_subs[sub] = true;
        }
        Ok(())
    }

    fn decode_network_data(&mut self, sub: usize, data: &[u8]) -> anyhow::Result<()> {
        // Try webp first.
        if let Ok(()) = self.decode_webp_data(sub, data) {
            Ok(())
        } else {
            self.decode_png_data(sub, data)
        }
    }

    fn mark_dirty(&mut self, sub: usize) {
        self.png_data[sub] = None;
        self.non_empty_subs[sub] = true;
        self.saved_subs[sub] = false;
    }

    fn mark_saved(&mut self, sub: usize) {
        self.saved_subs[sub] = true;
    }

    fn image_is_empty(image: &RgbaImage) -> bool {
        image.iter().all(|x| *x == 0)
    }

    fn image_info(size: (u32, u32)) -> ImageInfo {
        ImageInfo::new(
            ISize::new(size.0 as i32, size.1 as i32),
            skia::ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        )
    }
}

pub struct PaintCanvas {
    chunks: HashMap<(i32, i32), Chunk>,
    // this set contains all chunks that have already been visited in the current stroke() call
    stroked_chunks: HashSet<(i32, i32)>,
    filename: Option<PathBuf>,
}

pub const CANVAS_TOML_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct CanvasToml {
    version: u32,
    // more stuff to be added in the future
}

impl PaintCanvas {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            stroked_chunks: HashSet::new(),
            filename: None,
        }
    }

    fn ensure_chunk_exists(&mut self, canvas: &mut Canvas, position: (i32, i32)) {
        if !self.chunks.contains_key(&position) {
            self.chunks.insert(position, Chunk::new(canvas));
        }
    }

    pub fn stroke(&mut self, canvas: &mut Canvas, from: impl Into<Point>, to: impl Into<Point>, brush: &Brush) {
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

            for y in top_left_chunk.1..bottom_right_chunk.1 {
                for x in top_left_chunk.0..bottom_right_chunk.0 {
                    let chunk_position = (x, y);
                    let master = Chunk::master(chunk_position);
                    let sub = Chunk::sub(chunk_position);
                    if !self.stroked_chunks.contains(&master) {
                        self.ensure_chunk_exists(canvas, master);
                        let chunk = self.chunks.get_mut(&master).unwrap();
                        let screen_position = Chunk::screen_position(master);
                        chunk
                            .surface
                            .borrow_mut()
                            .canvas()
                            .draw_line(a - screen_position, b - screen_position, &paint);
                        chunk.mark_dirty(sub);
                    }
                    self.stroked_chunks.insert(master);
                    p.offset(delta);
                }
            }
        }
    }

    pub fn draw_to(&self, canvas: &mut Canvas, viewport: &Viewport, window_size: (f32, f32)) {
        for chunk_position in viewport.visible_tiles(Chunk::SURFACE_SIZE, window_size) {
            if let Some(chunk) = self.chunks.get(&chunk_position) {
                let screen_position = Chunk::screen_position(chunk_position);
                // why is the position parameter a Size? only rust-skia devs know.
                chunk.surface.borrow_mut().draw(
                    canvas,
                    (screen_position.x, screen_position.y),
                    SamplingOptions::default(),
                    None,
                );
            }
        }
    }

    pub fn network_data(&mut self, chunk_position: (i32, i32)) -> Option<&[u8]> {
        eprintln!("fetching data for network transmission from chunk {:?}", chunk_position);
        self.chunks
            .get_mut(&Chunk::master(chunk_position))?
            .network_data(Chunk::sub(chunk_position))
    }

    pub fn decode_network_data(&mut self, canvas: &mut Canvas, to_chunk: (i32, i32), data: &[u8]) -> anyhow::Result<()> {
        self.ensure_chunk_exists(canvas, Chunk::master(to_chunk));
        let chunk = self.chunks.get_mut(&Chunk::master(to_chunk)).unwrap();
        chunk.decode_network_data(Chunk::sub(to_chunk), data)
    }

    fn save_as_png(&self, path: &Path) -> anyhow::Result<()> {
        eprintln!("saving png {:?}", path);
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
        let width = ((right - left + 1) * Chunk::SURFACE_SIZE.0 as i32) as u32;
        let height = ((bottom - top + 1) * Chunk::SURFACE_SIZE.1 as i32) as u32;
        eprintln!("size: {:?}", (width, height));
        let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        for (chunk_position, chunk) in &self.chunks {
            eprintln!("writing chunk {:?}", chunk_position);
            let pixel_position = (
                (Chunk::SURFACE_SIZE.0 as i32 * (chunk_position.0 - left)) as u32,
                (Chunk::SURFACE_SIZE.1 as i32 * (chunk_position.1 - top)) as u32,
            );
            eprintln!("   - pixel position: {:?}", pixel_position);

            let chunk_image = chunk.download_image();
            let mut sub_image = image.sub_image(
                pixel_position.0,
                pixel_position.1,
                Chunk::SURFACE_SIZE.0 as u32,
                Chunk::SURFACE_SIZE.1 as u32,
            );
            sub_image.copy_from(&chunk_image, 0, 0)?;
        }
        image.save(path)?;
        eprintln!("image {:?} saved successfully", path);
        Ok(())
    }

    fn validate_netcanv_save_path(path: &Path) -> anyhow::Result<PathBuf> {
        // condition #1: remove canvas.toml
        let mut result = PathBuf::from(path);
        if result.file_name() == Some(OsStr::new("canvas.toml")) {
            result.pop();
        }
        // condition #2: make sure that the directory name ends with .netcanv
        if result.extension() != Some(OsStr::new("netcanv")) {
            anyhow::bail!("Please select a valid canvas folder (one whose name ends with .netcanv)")
        }
        Ok(result)
    }

    fn clear_netcanv_save(path: &Path) -> anyhow::Result<()> {
        eprintln!("clearing older netcanv save {:?}", path);
        for entry in std::fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_file() {
                if path.extension() == Some(OsStr::new("png")) || path.file_name() == Some(OsStr::new("canvas.toml")) {
                    std::fs::remove_file(path)?;
                }
            }
        }
        Ok(())
    }

    fn save_as_netcanv(&mut self, path: &Path) -> anyhow::Result<()> {
        // create the directory
        eprintln!("creating or reusing existing directory ({:?})", path);
        let path = Self::validate_netcanv_save_path(path)?;
        std::fs::create_dir_all(path.clone())?; // use create_dir_all to not fail if the dir already exists
        if self.filename != Some(path.clone()) {
            Self::clear_netcanv_save(&path)?;
        }
        // save the canvas.toml manifest
        eprintln!("saving canvas.toml");
        let canvas_toml = CanvasToml {
            version: CANVAS_TOML_VERSION,
        };
        std::fs::write(path.join(Path::new("canvas.toml")), toml::to_string(&canvas_toml)?)?;
        // save all the chunks
        eprintln!("saving chunks");
        for (master_position, chunk) in &mut self.chunks {
            for sub in 0..Chunk::SUB_COUNT {
                if !chunk.non_empty_subs[sub] || chunk.saved_subs[sub] {
                    continue
                }
                let chunk_position = Chunk::chunk_position(*master_position, sub);
                eprintln!("  chunk {:?}", chunk_position);
                let saved = if let Some(png_data) = chunk.png_data(sub) {
                    let filename = format!("{},{}.png", chunk_position.0, chunk_position.1);
                    let filepath = path.join(Path::new(&filename));
                    eprintln!("  saving to {:?}", filepath);
                    std::fs::write(filepath, png_data)?;
                    true
                } else {
                    false
                };
                if saved {
                    chunk.mark_saved(sub);
                }
            }
        }
        self.filename = Some(path);
        Ok(())
    }

    // if path is None, self.filename will be used
    pub fn save(&mut self, path: Option<&Path>) -> anyhow::Result<()> {
        let path = path
            .map(|p| p.to_path_buf())
            .or(self.filename.clone())
            .expect("no save path provided");
        if let Some(ext) = path.extension() {
            match ext.to_str() {
                Some("png") => self.save_as_png(&path),
                Some("netcanv") | Some("toml") => self.save_as_netcanv(&path),
                _ => anyhow::bail!("Unsupported save format. Please choose either .png or .netcanv"),
            }
        } else {
            anyhow::bail!("Can't save a canvas without an extension")
        }
    }

    fn extract_chunk_origin_from_filename(path: &Path) -> Option<(i32, i32)> {
        const ORG: &str = "!org";
        let filename = path.file_stem()?.to_str()?;
        let org_index = filename.rfind(ORG)?;
        let chunk_position = &filename[org_index + ORG.len()..];
        Self::parse_chunk_position(chunk_position).ok()
    }

    fn load_from_image_file(&mut self, canvas: &mut Canvas, path: &Path) -> anyhow::Result<()> {
        use ::image::io::Reader as ImageReader;

        let image = ImageReader::open(path)?.decode()?.into_rgba8();
        eprintln!("image size: {:?}", image.dimensions());
        let chunks_x = (image.width() as f32 / Chunk::SIZE.0 as f32).ceil() as i32;
        let chunks_y = (image.height() as f32 / Chunk::SIZE.1 as f32).ceil() as i32;
        eprintln!("n. chunks: x={}, y={}", chunks_x, chunks_y);
        let (origin_x, origin_y) = Self::extract_chunk_origin_from_filename(path).unwrap_or((0, 0));

        for y in 0..chunks_y {
            for x in 0..chunks_x {
                let chunk_position = (x, y);
                let offset_chunk_position = (x - origin_x, y - origin_y);
                // From the bottom of my heart, screw this stupid master-sub chunk system.
                // I wish Skia wasn't so slow at rendering large amounts of surfaces.
                let master_chunk = Chunk::master(offset_chunk_position);
                let sub_chunk = Chunk::sub_position(Chunk::sub(offset_chunk_position));
                self.ensure_chunk_exists(canvas, master_chunk);
                let chunk = self.chunks.get_mut(&master_chunk).unwrap();
                let pixel_position = (
                    Chunk::SIZE.0 * chunk_position.0 as u32,
                    Chunk::SIZE.1 * chunk_position.1 as u32,
                );
                eprintln!(
                    "plopping chunk at {:?} (master {:?} sub {:?} pxp {:?})",
                    offset_chunk_position, master_chunk, sub_chunk, pixel_position
                );
                let right = (pixel_position.0 + Chunk::SIZE.0).min(image.width() - 1);
                let bottom = (pixel_position.1 + Chunk::SIZE.1).min(image.height() - 1);
                let width = right - pixel_position.0;
                let height = bottom - pixel_position.1;
                let mut chunk_image = RgbaImage::from_pixel(Chunk::SIZE.0, Chunk::SIZE.1, Rgba([0, 0, 0, 0]));
                let sub_image = image.view(pixel_position.0, pixel_position.1, width, height);
                chunk_image.copy_from(&sub_image, 0, 0)?;
                if Chunk::image_is_empty(&chunk_image) {
                    continue
                }
                chunk.non_empty_subs = [true; Chunk::SUB_COUNT];
                chunk.upload_image(chunk_image, (sub_chunk.0 * Chunk::SIZE.0, sub_chunk.1 * Chunk::SIZE.1));
            }
        }

        Ok(())
    }

    fn parse_chunk_position(coords: &str) -> anyhow::Result<(i32, i32)> {
        let mut iter = coords.split(',');
        let x_str = iter.next();
        let y_str = iter.next();
        anyhow::ensure!(
            x_str.is_some() && y_str.is_some(),
            "Chunk position must follow the pattern: x,y"
        );
        anyhow::ensure!(iter.next().is_none(), "Trailing coordinates found after x,y");
        let (x_str, y_str) = (x_str.unwrap(), y_str.unwrap());
        let x: i32 = x_str.parse()?;
        let y: i32 = y_str.parse()?;
        Ok((x, y))
    }

    fn load_from_netcanv(&mut self, canvas: &mut Canvas, path: &Path) -> anyhow::Result<()> {
        let path = Self::validate_netcanv_save_path(path)?;
        eprintln!("loading canvas from {:?}", path);
        // load canvas.toml
        eprintln!("loading canvas.toml");
        let canvas_toml_path = path.join(Path::new("canvas.toml"));
        let canvas_toml: CanvasToml = toml::from_str(&std::fs::read_to_string(&canvas_toml_path)?)?;
        if canvas_toml.version < CANVAS_TOML_VERSION {
            anyhow::bail!("Version mismatch in canvas.toml. Try updating your client");
        }
        // load chunks
        eprintln!("loading chunks");
        for entry in std::fs::read_dir(path.clone())? {
            let path = entry?.path();
            if path.is_file() && path.extension() == Some(OsStr::new("png")) {
                if let Some(position_osstr) = path.file_stem() {
                    if let Some(position_str) = position_osstr.to_str() {
                        let chunk_position = Self::parse_chunk_position(&position_str)?;
                        eprintln!("chunk {:?}", chunk_position);
                        let master = Chunk::master(chunk_position);
                        let sub = Chunk::sub(chunk_position);
                        self.ensure_chunk_exists(canvas, master);
                        let chunk = self.chunks.get_mut(&master).unwrap();
                        chunk.decode_png_data(sub, &std::fs::read(path)?)?;
                        chunk.mark_saved(sub);
                    }
                }
            }
        }
        self.filename = Some(path);
        Ok(())
    }

    pub fn load(&mut self, canvas: &mut Canvas, path: &Path) -> anyhow::Result<()> {
        if let Some(ext) = path.extension() {
            match ext.to_str() {
                Some("netcanv") | Some("toml") => self.load_from_netcanv(canvas, path),
                _ => self.load_from_image_file(canvas, path),
            }
        } else {
            self.load_from_image_file(canvas, path)
        }
    }

    pub fn chunk_positions(&self) -> Vec<(i32, i32)> {
        let mut result = Vec::new();
        for (master_position, chunk) in &self.chunks {
            for (sub, non_empty) in chunk.non_empty_subs.iter().enumerate() {
                if *non_empty {
                    result.push(Chunk::chunk_position(*master_position, sub));
                }
            }
        }
        result
    }

    pub fn filename(&self) -> Option<&Path> {
        self.filename.as_deref()
    }
}
