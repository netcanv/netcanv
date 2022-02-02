//! NetCanv's infinite paint canvas.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ::image::codecs::png::{PngDecoder, PngEncoder};
use ::image::{
   ColorType, DynamicImage, GenericImage, GenericImageView, ImageBuffer, ImageDecoder, Rgba,
   RgbaImage,
};
use netcanv_renderer::paws::{vector, Color, Point, Rect, Renderer, Vector};
use netcanv_renderer::{Framebuffer as FramebufferTrait, RenderBackend};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::backend::{Backend, Framebuffer};
use crate::viewport::Viewport;

/// A chunk on the infinite canvas.
pub struct Chunk {
   framebuffer: Framebuffer,
   saved: bool,
}

impl Chunk {
   /// The maximum size threshold for a PNG to get converted to lossy WebP before network
   /// transmission.
   const MAX_PNG_SIZE: usize = 32 * 1024;
   /// The size of a sub-chunk.
   pub const SIZE: (u32, u32) = (256, 256);
   /// The quality of encoded WebP files.
   // Note to self in the future: the libwebp quality factor ranges from 0.0 to 100.0, not
   // from 0.0 to 1.0.
   // 80% is a fairly sane default that preserves most of the image's quality while still retaining a
   // good compression ratio.
   const WEBP_QUALITY: f32 = 80.0;

   /// Creates a new chunk, using the given canvas as a Skia surface allocator.
   fn new(renderer: &mut Backend) -> Self {
      Self {
         framebuffer: renderer.create_framebuffer(Self::SIZE.0, Self::SIZE.1),
         saved: true,
      }
   }

   /// Returns the on-screen position of the chunk at the given coordinates.
   fn screen_position(chunk_position: (i32, i32)) -> Point {
      Point::new(
         (chunk_position.0 * Self::SIZE.0 as i32) as _,
         (chunk_position.1 * Self::SIZE.1 as i32) as _,
      )
   }

   /// Downloads the image of the chunk from the graphics card.
   fn download_image(&self) -> RgbaImage {
      let mut image_buffer =
         ImageBuffer::from_pixel(Self::SIZE.0, Self::SIZE.1, Rgba([0, 0, 0, 0]));
      self.framebuffer.download_rgba((0, 0), self.framebuffer.size(), &mut image_buffer);
      image_buffer
   }

   /// Uploads the image of the chunk to the graphics card, at the given offset in the master
   /// chunk.
   fn upload_image(&mut self, image: &RgbaImage, offset: (u32, u32)) {
      self.mark_dirty();
      self.framebuffer.upload_rgba(offset, Self::SIZE, &image);
   }

   /// Encodes an image to PNG data asynchronously.
   async fn encode_png_data(image: RgbaImage) -> anyhow::Result<Vec<u8>> {
      Ok(tokio::task::spawn_blocking(move || {
         let mut bytes: Vec<u8> = Vec::new();
         match PngEncoder::new(Cursor::new(&mut bytes)).encode(
            &image,
            image.width(),
            image.height(),
            ColorType::Rgba8,
         ) {
            Ok(()) => (),
            Err(error) => {
               log::error!("error while encoding: {}", error);
               anyhow::bail!(error)
            }
         }
         Ok(bytes)
      })
      .await??)
   }

   /// Encodes an image to WebP asynchronously.
   async fn encode_webp_data(image: RgbaImage) -> anyhow::Result<Vec<u8>> {
      Ok(tokio::task::spawn_blocking(move || {
         let image = DynamicImage::ImageRgba8(image);
         let encoder = webp::Encoder::from_image(&image).unwrap();
         encoder.encode(Self::WEBP_QUALITY).to_owned()
      })
      .await?)
   }

   /// Encodes a network image asynchronously. This encodes PNG, as well as WebP if the PNG is too
   /// large, and returns both images.
   async fn encode_network_data(image: RgbaImage) -> anyhow::Result<(Vec<u8>, Option<Vec<u8>>)> {
      let png_data = Self::encode_png_data(image.clone()).await?;
      let webp_data = if png_data.len() > Self::MAX_PNG_SIZE {
         Some(Self::encode_webp_data(image).await?)
      } else {
         None
      };
      Ok((png_data, webp_data))
   }

   /// Decodes a PNG file into the given sub-chunk.
   fn decode_png_data(data: &[u8]) -> anyhow::Result<RgbaImage> {
      let decoder = PngDecoder::new(Cursor::new(data))?;
      if decoder.color_type() != ColorType::Rgba8 {
         log::warn!("received non-RGBA image data, ignoring");
         anyhow::bail!("non-RGBA chunk image");
      }
      let mut image = RgbaImage::from_pixel(Self::SIZE.0, Self::SIZE.1, Rgba([0, 0, 0, 0]));
      decoder.read_image(&mut image)?;
      Ok(image)
   }

   /// Decodes a WebP file into the given sub-chunk.
   fn decode_webp_data(data: &[u8]) -> anyhow::Result<RgbaImage> {
      let decoder = webp::Decoder::new(data);
      let image = match decoder.decode() {
         Some(image) => image.to_image(),
         None => anyhow::bail!("got non-webp image"),
      }
      .into_rgba8();
      Ok(image)
   }

   /// Decodes a PNG or WebP file into the given sub-chunk, depending on what's actually stored in
   /// `data`.
   fn decode_network_data(data: &[u8]) -> anyhow::Result<RgbaImage> {
      // Try WebP first.
      let image = Self::decode_webp_data(data).or_else(|_| Self::decode_png_data(data))?;
      if image.dimensions() != Self::SIZE {
         log::error!(
            "received chunk with invalid size. got: {:?}, expected {:?}",
            image.dimensions(),
            Self::SIZE
         );
         anyhow::bail!("invalid chunk image size");
      }
      Ok(image)
   }

   /// Marks the given sub-chunk within this master chunk as dirty - that is, invalidates any
   /// cached PNG and WebP data, marks the sub-chunk as non-empty, and marks it as unsaved.
   fn mark_dirty(&mut self) {}

   /// Marks the given sub-chunk within this master chunk as saved.
   fn mark_saved(&mut self) {
      self.saved = true;
   }

   /// Iterates through all pixels within the image and checks whether any pixels in the image are
   /// not transparent.
   fn image_is_empty(image: &RgbaImage) -> bool {
      image.iter().all(|x| *x == 0)
   }
}

/// A paint canvas built out of [`Chunk`]s.
pub struct PaintCanvas {
   chunks: HashMap<(i32, i32), Chunk>,
   /// The path to the `.netcanv` directory this paint canvas was saved to.
   filename: Option<PathBuf>,

   runtime: Arc<Runtime>,
   chunks_to_decode_tx: mpsc::UnboundedSender<((i32, i32), Vec<u8>)>,
   decoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), RgbaImage)>,
   decoder_quitter: Option<(oneshot::Sender<()>, JoinHandle<()>)>,
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
   /// Creates a new, empty paint canvas.
   pub fn new(runtime: Arc<Runtime>) -> Self {
      // Set up decoding supervisor thread.
      let (to_decode_tx, to_decode_rx) = mpsc::unbounded_channel();
      let (decoded_chunks_tx, decoded_chunks_rx) = mpsc::unbounded_channel();
      let (decoder_quit_tx, decoder_quit_rx) = oneshot::channel();
      let runtime2 = Arc::clone(&runtime);
      let decoder_join_handle = runtime.spawn(async move {
         Self::chunk_decoding_loop(runtime2, to_decode_rx, decoded_chunks_tx, decoder_quit_rx)
            .await;
      });

      Self {
         chunks: HashMap::new(),
         filename: None,
         runtime,
         chunks_to_decode_tx: to_decode_tx,
         decoded_chunks_rx,
         decoder_quitter: Some((decoder_quit_tx, decoder_join_handle)),
      }
   }

   /// Creates the chunk at the given position, if it doesn't already exist.
   fn ensure_chunk_exists(&mut self, renderer: &mut Backend, position: (i32, i32)) {
      if !self.chunks.contains_key(&position) {
         self.chunks.insert(position, Chunk::new(renderer));
      }
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
            let master_chunk = (x, y);
            self.ensure_chunk_exists(renderer, master_chunk);
            let chunk = self.chunks.get_mut(&master_chunk).unwrap();
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

   /// The decoding supervisor thread.
   async fn chunk_decoding_loop(
      runtime: Arc<Runtime>,
      mut input: mpsc::UnboundedReceiver<((i32, i32), Vec<u8>)>,
      output: mpsc::UnboundedSender<((i32, i32), RgbaImage)>,
      mut quit: oneshot::Receiver<()>,
   ) {
      log::info!("starting chunk decoding supervisor thread");
      loop {
         tokio::select! {
            biased;
            Ok(_) = &mut quit => break,
            data = input.recv() => {
               if let Some((chunk_position, image_data)) = data {
                  let output = output.clone();
                  runtime.spawn_blocking(move || match Chunk::decode_network_data(&image_data) {
                     Ok(image) => {
                        // Doesn't matter if the receiving half is closed.
                        let _ = output.send((chunk_position, image));
                     }
                     Err(error) => log::error!("image decoding failed: {}", error),
                  });
               } else {
                  log::info!("decoding supervisor: chunk data sender was dropped, quitting");
                  break;
               }
            },
         }
      }
      log::info!("exiting chunk decoding supervisor thread");
   }

   /// Updates chunks that have been decoded between the last call to `update` and the current one.
   pub fn update(&mut self, renderer: &mut Backend) {
      while let Ok((chunk_position, image)) = self.decoded_chunks_rx.try_recv() {
         self.ensure_chunk_exists(renderer, chunk_position);
         let chunk = self.chunks.get_mut(&chunk_position).unwrap();
         chunk.upload_image(&image, (0, 0));
      }
   }

   /// Returns a receiver for the image data of the chunk at the given position, if it's not empty.
   ///
   /// The chunk data that arrives from the receiver may be `None` if encoding failed.
   pub fn enqueue_network_data_encoding(
      &mut self,
      output_channel: mpsc::UnboundedSender<((i32, i32), Vec<u8>)>,
      chunk_position: (i32, i32),
   ) {
      log::info!(
         "fetching data for network transmission of chunk {:?}",
         chunk_position
      );
      if let Some(chunk) = self.chunks.get_mut(&chunk_position) {
         let image = chunk.download_image();
         if Chunk::image_is_empty(&image) {
            return;
         }
         self.runtime.spawn(async move {
            log::debug!("encoding image data for chunk {:?}", chunk_position);
            let image_data = Chunk::encode_network_data(image).await;
            log::debug!("encoding done for chunk {:?}", chunk_position);
            match image_data {
               Ok((_png, Some(webp))) => {
                  log::debug!("sending webp data back to main thread");
                  let _ = output_channel.send((chunk_position, webp));
               }
               Ok((png, None)) => {
                  log::debug!("sending png data back to main thread");
                  let _ = output_channel.send((chunk_position, png));
               }
               Err(error) => {
                  log::error!(
                     "error while encoding image for chunk {:?}: {}",
                     chunk_position,
                     error
                  );
               }
            }
         });
      }
   }

   /// Enqueues image data for decoding to the chunk at the given position.
   pub fn enqueue_network_data_decoding(
      &mut self,
      to_chunk: (i32, i32),
      data: Vec<u8>,
   ) -> anyhow::Result<()> {
      self
         .chunks_to_decode_tx
         .send((to_chunk, data))
         .expect("Decoding supervisor thread should never quit");
      // chunk.decode_network_data(Chunk::sub(to_chunk), data)
      Ok(())
   }

   /// Saves the entire paint to a PNG file.
   fn save_as_png(&self, path: &Path) -> anyhow::Result<()> {
      log::info!("saving png {:?}", path);
      let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
      for (chunk_position, _) in &self.chunks {
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
         anyhow::bail!("There's nothing to save! Draw something on the canvas and try again.");
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

   /// Clears the existing `.netcanv` save at the given path.
   fn clear_netcanv_save(path: &Path) -> anyhow::Result<()> {
      log::info!("clearing older netcanv save {:?}", path);
      for entry in std::fs::read_dir(path)? {
         let path = entry?.path();
         if path.is_file() {
            if path.extension() == Some(OsStr::new("png"))
               || path.file_name() == Some(OsStr::new("canvas.toml"))
            {
               std::fs::remove_file(path)?;
            }
         }
      }
      Ok(())
   }

   /// Saves the paint canvas as a `.netcanv` canvas.
   async fn save_as_netcanv(&mut self, path: &Path) -> anyhow::Result<()> {
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
         let image_data = Chunk::encode_png_data(image).await?;
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
   pub fn save(&mut self, path: Option<&Path>) -> anyhow::Result<()> {
      let path =
         path.map(|p| p.to_path_buf()).or(self.filename.clone()).expect("no save path provided");
      if let Some(ext) = path.extension() {
         match ext.to_str() {
            Some("png") => self.save_as_png(&path),
            Some("netcanv") | Some("toml") => {
               let runtime = Arc::clone(&self.runtime);
               runtime.block_on(async { self.save_as_netcanv(&path).await })
            }
            _ => anyhow::bail!("Unsupported save format. Please choose either .png or .netcanv"),
         }
      } else {
         anyhow::bail!("Can't save a canvas without an extension")
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
   fn load_from_image_file(&mut self, renderer: &mut Backend, path: &Path) -> anyhow::Result<()> {
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
            self.ensure_chunk_exists(renderer, offset_chunk_position);
            let chunk = self.chunks.get_mut(&offset_chunk_position).unwrap();
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
   fn parse_chunk_position(coords: &str) -> anyhow::Result<(i32, i32)> {
      let mut iter = coords.split(',');
      let x_str = iter.next();
      let y_str = iter.next();
      anyhow::ensure!(
         x_str.is_some() && y_str.is_some(),
         "Chunk position must follow the pattern: x,y"
      );
      anyhow::ensure!(
         iter.next().is_none(),
         "Trailing coordinates found after x,y"
      );
      let (x_str, y_str) = (x_str.unwrap(), y_str.unwrap());
      let x: i32 = x_str.parse()?;
      let y: i32 = y_str.parse()?;
      Ok((x, y))
   }

   /// Loads chunks from a `.netcanv` directory.
   fn load_from_netcanv(&mut self, renderer: &mut Backend, path: &Path) -> anyhow::Result<()> {
      let path = Self::validate_netcanv_save_path(path)?;
      log::info!("loading canvas from {:?}", path);
      // load canvas.toml
      log::debug!("loading canvas.toml");
      let canvas_toml_path = path.join(Path::new("canvas.toml"));
      let canvas_toml: CanvasToml = toml::from_str(&std::fs::read_to_string(&canvas_toml_path)?)?;
      if canvas_toml.version < CANVAS_TOML_VERSION {
         anyhow::bail!("Version mismatch in canvas.toml. Try updating your client");
      }
      // load chunks
      log::debug!("loading chunks");
      for entry in std::fs::read_dir(path.clone())? {
         let path = entry?.path();
         // Please let me have if let chains.
         if path.is_file() && path.extension() == Some(OsStr::new("png")) {
            if let Some(position_osstr) = path.file_stem() {
               if let Some(position_str) = position_osstr.to_str() {
                  let chunk_position = Self::parse_chunk_position(&position_str)?;
                  log::debug!("chunk {:?}", chunk_position);
                  self.ensure_chunk_exists(renderer, chunk_position);
                  let chunk = self.chunks.get_mut(&chunk_position).unwrap();
                  let image_data = Chunk::decode_png_data(&std::fs::read(path)?)?;
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
   pub fn load(&mut self, renderer: &mut Backend, path: &Path) -> anyhow::Result<()> {
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

impl Drop for PaintCanvas {
   fn drop(&mut self) {
      self.runtime.block_on(async {
         let (channel, join_handle) = self.decoder_quitter.take().unwrap();
         let _ = channel.send(());
         let _ = join_handle.await;
      });
   }
}
