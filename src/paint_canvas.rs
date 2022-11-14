//! NetCanv's infinite paint canvas.

pub mod chunk;

use std::collections::HashMap;

use ::image::RgbaImage;
use instant::{Duration, Instant};
use netcanv_renderer::paws::{vector, Color, Rect, Renderer, Vector};
use netcanv_renderer::{Framebuffer as FramebufferTrait, RenderBackend};
use tokio::sync::mpsc;

use crate::backend::{Backend, Framebuffer};
use chunk::{Chunk, ChunkImage};
use crate::viewport::Viewport;
use crate::image_coder::ImageCoder;

/// A paint canvas built out of [`Chunk`]s.
pub struct PaintCanvas {
   chunks: HashMap<(i32, i32), Chunk>,

   xcoder: ImageCoder,

   decoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), RgbaImage)>,
   encoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), ChunkImage)>,

   chunk_cache_timers: HashMap<(i32, i32), Instant>,
}

impl PaintCanvas {
   /// The duration for which encoded chunk images are held in memory.
   /// Once this duration expires, the cached images are dropped.
   const CHUNK_CACHE_DURATION: Duration = Duration::from_secs(5 * 60);

   /// Creates a new, empty paint canvas.
   pub fn new(
      xcoder: ImageCoder,
      decoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), RgbaImage)>,
      encoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), ChunkImage)>,
   ) -> Self {
      Self {
         chunks: HashMap::new(),

         xcoder,

         decoded_chunks_rx,
         encoded_chunks_rx,

         chunk_cache_timers: HashMap::new(),
      }
   }

   /// Creates the chunk at the given position, if it doesn't already exist.
   #[must_use]
   pub fn ensure_chunk(&mut self, renderer: &mut Backend, position: (i32, i32)) -> &mut Chunk {
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

   pub fn chunks(&self) -> &HashMap<(i32, i32), Chunk> {
      &self.chunks
   }

   pub fn chunks_mut(&mut self) -> &mut HashMap<(i32, i32), Chunk> {
      &mut self.chunks
   }

   /// Returns a vector containing all the chunk positions in the paint canvas.
   pub fn chunk_positions(&self) -> Vec<(i32, i32)> {
      self.chunks.keys().copied().collect()
   }
}
