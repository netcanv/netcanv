//! NetCanv's infinite paint canvas.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ::image::{GenericImage, GenericImageView, Rgba, RgbaImage};
use instant::{Duration, Instant};
use netcanv_renderer::paws::{vector, Color, Rect, Renderer, Vector};
use netcanv_renderer::{Framebuffer as FramebufferTrait, RenderBackend};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::backend::{Backend, Framebuffer};
use crate::chunk::{Chunk, ChunkImage};
use crate::viewport::Viewport;
use crate::xcoder::Xcoder;
use crate::Error;

/// A paint canvas built out of [`Chunk`]s.
pub struct PaintCanvas {
   chunks: HashMap<(i32, i32), Chunk>,
   /// The path to the `.netcanv` directory this paint canvas was saved to.
   filename: Option<PathBuf>,

   runtime: Arc<Runtime>,
   xcoder: Xcoder,

   decoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), RgbaImage)>,
   encoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), ChunkImage)>,

   chunk_cache_timers: HashMap<(i32, i32), Instant>,
}

/// The format version in a `.netcanv`'s `canvas.toml` file.
pub const CANVAS_TOML_VERSION: u32 = 1;

/// A `canvas.toml` file.
#[derive(Serialize, Deserialize)]
struct CanvasToml {
   /// The format version of the canvas.
   version: u32,
}

impl PaintCanvas {
   /// The duration for which encoded chunk images are held in memory.
   /// Once this duration expires, the cached images are dropped.
   const CHUNK_CACHE_DURATION: Duration = Duration::from_secs(5 * 60);

   /// Creates a new, empty paint canvas.
   pub fn new(
      runtime: Arc<Runtime>,
      xcoder: Xcoder,
      decoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), RgbaImage)>,
      encoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), ChunkImage)>,
   ) -> Self {
      Self {
         chunks: HashMap::new(),
         filename: None,

         runtime,
         xcoder,

         decoded_chunks_rx,
         encoded_chunks_rx,

         chunk_cache_timers: HashMap::new(),
      }
   }

   /// Creates the chunk at the given position, if it doesn't already exist.
   #[must_use]
   fn ensure_chunk(&mut self, renderer: &mut Backend, position: (i32, i32)) -> &mut Chunk {
      self.chunks.entry(position).or_insert_with(|| Chunk::new(renderer))
   }

   /// Returns the left, top, bottom, right sides covered by the rectangle, in chunk
   /// coordinates.
   fn chunk_coverage(coverage: Rect) -> (i32, i32, i32, i32) {
      let coverage = coverage.sort();
      (
         (coverage.left() / Chunk::SIZE.0 as f32).floor() as i32,
         (coverage.top() / Chunk::SIZE.1 as f32).floor() as i32,
         (coverage.bottom() / Chunk::SIZE.0 as f32).floor() as i32,
         (coverage.right() / Chunk::SIZE.1 as f32).floor() as i32,
      )
   }

   /// Draws to the paint canvas's chunks.
   ///
   /// The provided `coverage` rectangle is used to determine which chunks should be drawn to, and
   /// thus should cover the entire area of the thing being drawn. Note that the coordinates here
   /// are expressed in _pixels_ rather than _chunks_.
   ///
   /// The callback may be called multiple times, once for each chunk being drawn to.
   pub fn draw(
      &mut self,
      renderer: &mut Backend,
      coverage: Rect,
      mut callback: impl FnMut(&mut Backend),
   ) {
      let (left, top, bottom, right) = Self::chunk_coverage(coverage);
      assert!(left <= right);
      assert!(top <= bottom);
      for y in top..=bottom {
         for x in left..=right {
            let chunk_position = (x, y);
            let chunk = self.ensure_chunk(renderer, chunk_position);
            renderer.push();
            renderer.translate(vector(
               -x as f32 * Chunk::SIZE.0 as f32,
               -y as f32 * Chunk::SIZE.0 as f32,
            ));
            renderer.draw_to(&chunk.framebuffer, |renderer| {
               callback(renderer);
            });
            renderer.pop();
            chunk.mark_dirty();
         }
      }
   }

   /// Captures a fragment of the paint canvas onto a framebuffer.
   pub fn capture(&self, renderer: &mut Backend, framebuffer: &Framebuffer, viewport: &Viewport) {
      renderer.draw_to(framebuffer, |renderer| {
         self.draw_to(
            renderer,
            viewport,
            vector(framebuffer.width() as f32, framebuffer.height() as f32),
         );
      });
   }

   /// Downloads the color of the pixel at the provided position.
   pub fn get_pixel(&self, position: (i64, i64)) -> Color {
      if let Some(chunk) = self.chunks.get(&(
         (position.0.div_euclid(Chunk::SIZE.0 as i64)) as i32,
         (position.1.div_euclid(Chunk::SIZE.1 as i64)) as i32,
      )) {
         let position_in_chunk = (
            (position.0.rem_euclid(Chunk::SIZE.0 as i64)) as u32,
            (position.1.rem_euclid(Chunk::SIZE.1 as i64)) as u32,
         );
         let mut rgba = [0u8; 4];
         chunk.framebuffer.download_rgba(position_in_chunk, (1, 1), &mut rgba);
         let [r, g, b, a] = rgba;
         Color { r, g, b, a }
      } else {
         Color::TRANSPARENT
      }
   }

   /// Draws the paint canvas using the given renderer.
   ///
   /// The provided viewport and window size are used to only render chunks that are visible at a
   /// given moment.
   pub fn draw_to(&self, renderer: &mut Backend, viewport: &Viewport, window_size: Vector) {
      for chunk_position in viewport.visible_tiles(Chunk::SIZE, window_size) {
         if let Some(chunk) = self.chunks.get(&chunk_position) {
            let screen_position = Chunk::screen_position(chunk_position);
            renderer.framebuffer(chunk.framebuffer.rect(screen_position), &chunk.framebuffer);
         }
      }
   }

   /// Updates chunks that have been decoded between the last call to `update` and the current one.
   pub fn update(&mut self, renderer: &mut Backend) {
      while let Ok((chunk_position, image)) = self.decoded_chunks_rx.try_recv() {
         let chunk = self.ensure_chunk(renderer, chunk_position);
         chunk.upload_image(&image, (0, 0));
      }
      while let Ok((chunk_position, image)) = self.encoded_chunks_rx.try_recv() {
         let chunk = self.ensure_chunk(renderer, chunk_position);
         chunk.image_cache = Some(image);
         self.chunk_cache_timers.insert(chunk_position, Instant::now());
      }
      for (chunk_position, instant) in &self.chunk_cache_timers {
         if instant.elapsed() > Self::CHUNK_CACHE_DURATION {
            if let Some(chunk) = self.chunks.get_mut(chunk_position) {
               chunk.image_cache = None;
            }
         }
      }
   }

   /// Returns a receiver for the image data of the chunk at the given position, if it's not empty.
   ///
   /// The chunk data that arrives from the receiver may be `None` if encoding failed.
   pub fn enqueue_network_data_encoding(
      &mut self,
      output_channel: mpsc::UnboundedSender<((i32, i32), ChunkImage)>,
      chunk_position: (i32, i32),
   ) {
      log::info!(
         "fetching data for network transmission of chunk {:?}",
         chunk_position
      );
      if let Some(chunk) = self.chunks.get_mut(&chunk_position) {
         // Reset timers for recently accessed chunks.
         if chunk.image_cache.is_some() {
            self.chunk_cache_timers.insert(chunk_position, Instant::now());
         }

         self.xcoder.enqueue_chunk_encoding(chunk, output_channel, chunk_position);
      }
   }

   /// Enqueues image data for decoding to the chunk at the given position.
   pub fn enqueue_network_data_decoding(
      &mut self,
      to_chunk: (i32, i32),
      data: Vec<u8>,
   ) -> netcanv::Result<()> {
      self.xcoder.enqueue_chunk_decoding(to_chunk, data);
      // chunk.decode_network_data(Chunk::sub(to_chunk), data)
      Ok(())
   }

   /// Saves the entire paint to a PNG file.
   fn save_as_png(&self, path: &Path) -> netcanv::Result<()> {
      log::info!("saving png {:?}", path);
      let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
      for chunk_position in self.chunks.keys() {
         left = left.min(chunk_position.0);
         top = top.min(chunk_position.1);
         right = right.max(chunk_position.0);
         bottom = bottom.max(chunk_position.1);
      }
      log::debug!(
         "left={}, top={}, right={}, bottom={}",
         left,
         top,
         right,
         bottom
      );
      if left == i32::MAX {
         return Err(Error::NothingToSave);
      }
      let width = ((right - left + 1) * Chunk::SIZE.0 as i32) as u32;
      let height = ((bottom - top + 1) * Chunk::SIZE.1 as i32) as u32;
      log::debug!("size: {:?}", (width, height));
      let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
      for (chunk_position, chunk) in &self.chunks {
         log::debug!("writing chunk {:?}", chunk_position);
         let pixel_position = (
            (Chunk::SIZE.0 as i32 * (chunk_position.0 - left)) as u32,
            (Chunk::SIZE.1 as i32 * (chunk_position.1 - top)) as u32,
         );
         log::debug!("   - pixel position: {:?}", pixel_position);

         let chunk_image = chunk.download_image();
         let mut sub_image = image.sub_image(
            pixel_position.0,
            pixel_position.1,
            Chunk::SIZE.0 as u32,
            Chunk::SIZE.1 as u32,
         );
         sub_image.copy_from(&chunk_image, 0, 0)?;
      }
      image.save(path)?;
      log::debug!("image {:?} saved successfully", path);
      Ok(())
   }

   /// Validates the `.netcanv` save path. This strips away the `canvas.toml` if present, and makes
   /// sure that the directory name ends with `.netcanv`.
   fn validate_netcanv_save_path(path: &Path) -> netcanv::Result<PathBuf> {
      // condition #1: remove canvas.toml
      let mut result = PathBuf::from(path);
      if result.file_name() == Some(OsStr::new("canvas.toml")) {
         result.pop();
      }
      // condition #2: make sure that the directory name ends with .netcanv
      if result.extension() != Some(OsStr::new("netcanv")) {
         return Err(Error::InvalidCanvasFolder);
      }
      Ok(result)
   }

   /// Clears the existing `.netcanv` save at the given path.
   fn clear_netcanv_save(path: &Path) -> netcanv::Result<()> {
      log::info!("clearing older netcanv save {:?}", path);
      for entry in std::fs::read_dir(path)? {
         let path = entry?.path();
         if path.is_file()
            && (path.extension() == Some(OsStr::new("png"))
               || path.file_name() == Some(OsStr::new("canvas.toml")))
         {
            std::fs::remove_file(path)?;
         }
      }
      Ok(())
   }

   /// Saves the paint canvas as a `.netcanv` canvas.
   async fn save_as_netcanv(&mut self, path: &Path) -> netcanv::Result<()> {
      // create the directory
      log::info!("creating or reusing existing directory ({:?})", path);
      let path = Self::validate_netcanv_save_path(path)?;
      std::fs::create_dir_all(path.clone())?; // use create_dir_all to not fail if the dir already exists
      if self.filename != Some(path.clone()) {
         Self::clear_netcanv_save(&path)?;
      }
      // save the canvas.toml manifest
      log::info!("saving canvas.toml");
      let canvas_toml = CanvasToml {
         version: CANVAS_TOML_VERSION,
      };
      std::fs::write(
         path.join(Path::new("canvas.toml")),
         toml::to_string(&canvas_toml)?,
      )?;
      // save all the chunks
      log::info!("saving chunks");
      for (chunk_position, chunk) in &mut self.chunks {
         log::debug!("chunk {:?}", chunk_position);
         let image = chunk.download_image();
         let image_data = Xcoder::encode_png_data(image).await?;
         let filename = format!("{},{}.png", chunk_position.0, chunk_position.1);
         let filepath = path.join(Path::new(&filename));
         log::debug!("saving to {:?}", filepath);
         std::fs::write(filepath, image_data)?;
         chunk.mark_saved();
      }
      self.filename = Some(path);
      Ok(())
   }

   /// Saves the canvas to a PNG file or a `.netcanv` directory.
   ///
   /// If `path` is `None`, this performs an autosave of an already saved `.netcanv` directory.
   pub fn save(&mut self, path: Option<&Path>) -> netcanv::Result<()> {
      let path = path
         .map(|p| p.to_path_buf())
         .or_else(|| self.filename.clone())
         .expect("no save path provided");
      if let Some(ext) = path.extension() {
         match ext.to_str() {
            Some("png") => self.save_as_png(&path),
            Some("netcanv") | Some("toml") => {
               let runtime = Arc::clone(&self.runtime);
               runtime.block_on(async { self.save_as_netcanv(&path).await })
            }
            _ => Err(Error::UnsupportedSaveFormat),
         }
      } else {
         Err(Error::MissingCanvasSaveExtension)
      }
   }

   /// Extracts the `!org` origin part from an image file's name.
   fn extract_chunk_origin_from_filename(path: &Path) -> Option<(i32, i32)> {
      const ORG: &str = "!org";
      let filename = path.file_stem()?.to_str()?;
      let org_index = filename.rfind(ORG)?;
      let chunk_position = &filename[org_index + ORG.len()..];
      Self::parse_chunk_position(chunk_position).ok()
   }

   /// Loads chunks from an image file.
   fn load_from_image_file(&mut self, renderer: &mut Backend, path: &Path) -> netcanv::Result<()> {
      use ::image::io::Reader as ImageReader;

      let image = ImageReader::open(path)?.decode()?.into_rgba8();
      log::debug!("image size: {:?}", image.dimensions());
      let chunks_x = (image.width() as f32 / Chunk::SIZE.0 as f32).ceil() as i32;
      let chunks_y = (image.height() as f32 / Chunk::SIZE.1 as f32).ceil() as i32;
      log::debug!("n. chunks: x={}, y={}", chunks_x, chunks_y);
      let (origin_x, origin_y) = Self::extract_chunk_origin_from_filename(path).unwrap_or((0, 0));

      for y in 0..chunks_y {
         for x in 0..chunks_x {
            let chunk_position = (x, y);
            let offset_chunk_position = (x - origin_x, y - origin_y);
            let chunk = self.ensure_chunk(renderer, offset_chunk_position);
            let pixel_position = (
               Chunk::SIZE.0 * chunk_position.0 as u32,
               Chunk::SIZE.1 * chunk_position.1 as u32,
            );
            log::debug!(
               "plopping chunk at {:?} (pxp {:?})",
               offset_chunk_position,
               pixel_position
            );
            let right = (pixel_position.0 + Chunk::SIZE.0).min(image.width() - 1);
            let bottom = (pixel_position.1 + Chunk::SIZE.1).min(image.height() - 1);
            let width = right - pixel_position.0;
            let height = bottom - pixel_position.1;
            let mut chunk_image =
               RgbaImage::from_pixel(Chunk::SIZE.0, Chunk::SIZE.1, Rgba([0, 0, 0, 0]));
            let sub_image = image.view(pixel_position.0, pixel_position.1, width, height);
            chunk_image.copy_from(&sub_image, 0, 0)?;
            if Chunk::image_is_empty(&chunk_image) {
               continue;
            }
            chunk.mark_dirty();
            chunk.upload_image(&chunk_image, (0, 0));
         }
      }

      Ok(())
   }

   /// Parses an `x,y` chunk position.
   fn parse_chunk_position(coords: &str) -> netcanv::Result<(i32, i32)> {
      let mut iter = coords.split(',');
      let x_str = iter.next();
      let y_str = iter.next();
      ensure!(
         x_str.is_some() && y_str.is_some(),
         Error::InvalidChunkPositionPattern
      );
      ensure!(
         iter.next().is_none(),
         Error::TrailingChunkCoordinatesInFilename
      );
      let (x_str, y_str) = (x_str.unwrap(), y_str.unwrap());
      let x: i32 = x_str.parse()?;
      let y: i32 = y_str.parse()?;
      Ok((x, y))
   }

   /// Loads chunks from a `.netcanv` directory.
   fn load_from_netcanv(&mut self, renderer: &mut Backend, path: &Path) -> netcanv::Result<()> {
      let path = Self::validate_netcanv_save_path(path)?;
      log::info!("loading canvas from {:?}", path);
      // load canvas.toml
      log::debug!("loading canvas.toml");
      let canvas_toml_path = path.join(Path::new("canvas.toml"));
      let canvas_toml: CanvasToml = toml::from_str(&std::fs::read_to_string(&canvas_toml_path)?)?;
      if canvas_toml.version > CANVAS_TOML_VERSION {
         return Err(Error::CanvasTomlVersionMismatch);
      }
      // load chunks
      log::debug!("loading chunks");
      for entry in std::fs::read_dir(path.clone())? {
         let path = entry?.path();
         // Please let me have if let chains.
         if path.is_file() && path.extension() == Some(OsStr::new("png")) {
            if let Some(position_osstr) = path.file_stem() {
               if let Some(position_str) = position_osstr.to_str() {
                  let chunk_position = Self::parse_chunk_position(position_str)?;
                  log::debug!("chunk {:?}", chunk_position);
                  let chunk = self.ensure_chunk(renderer, chunk_position);
                  let image_data = Xcoder::decode_png_data(&std::fs::read(path)?)?;
                  chunk.upload_image(&image_data, (0, 0));
                  chunk.mark_saved();
               }
            }
         }
      }
      self.filename = Some(path);
      Ok(())
   }

   /// Loads a paint canvas from the given path.
   pub fn load(&mut self, renderer: &mut Backend, path: &Path) -> netcanv::Result<()> {
      if let Some(ext) = path.extension() {
         match ext.to_str() {
            Some("netcanv") | Some("toml") => self.load_from_netcanv(renderer, path),
            _ => self.load_from_image_file(renderer, path),
         }
      } else {
         self.load_from_image_file(renderer, path)
      }
   }

   /// Returns a vector containing all the chunk positions in the paint canvas.
   pub fn chunk_positions(&self) -> Vec<(i32, i32)> {
      self.chunks.keys().copied().collect()
   }

   /// Returns what filename the canvas was saved under.
   pub fn filename(&self) -> Option<&Path> {
      self.filename.as_deref()
   }
}
