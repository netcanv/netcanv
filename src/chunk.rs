use std::io::Cursor;

use ::image::codecs::png::{PngDecoder, PngEncoder};
use ::image::{
   ColorType, DynamicImage, ImageBuffer, ImageDecoder, Rgba,
   RgbaImage,
};
use netcanv_renderer::paws::Point;
use netcanv_renderer::{Framebuffer as FramebufferTrait, RenderBackend};

use crate::backend::{Backend, Framebuffer};
use crate::Error;

#[derive(Clone)]
pub struct ChunkImage {
   pub png: Vec<u8>,
   pub webp: Option<Vec<u8>>,
}

/// A chunk on the infinite canvas.
pub struct Chunk {
   pub framebuffer: Framebuffer,
   pub image_cache: Option<ChunkImage>,
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
   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         framebuffer: renderer.create_framebuffer(Self::SIZE.0, Self::SIZE.1),
         image_cache: None,
         saved: true,
      }
   }

   /// Returns the on-screen position of the chunk at the given coordinates.
   pub fn screen_position(chunk_position: (i32, i32)) -> Point {
      Point::new(
         (chunk_position.0 * Self::SIZE.0 as i32) as _,
         (chunk_position.1 * Self::SIZE.1 as i32) as _,
      )
   }

   /// Downloads the image of the chunk from the graphics card.
   pub fn download_image(&self) -> RgbaImage {
      let mut image_buffer =
         ImageBuffer::from_pixel(Self::SIZE.0, Self::SIZE.1, Rgba([0, 0, 0, 0]));
      self.framebuffer.download_rgba((0, 0), self.framebuffer.size(), &mut image_buffer);
      image_buffer
   }

   /// Uploads the image of the chunk to the graphics card, at the given offset in the master
   /// chunk.
   pub fn upload_image(&mut self, image: &RgbaImage, offset: (u32, u32)) {
      self.mark_dirty();
      self.framebuffer.upload_rgba(offset, Self::SIZE, image);
   }

   /// Encodes an image to PNG data asynchronously.
   pub async fn encode_png_data(image: RgbaImage) -> netcanv::Result<Vec<u8>> {
      tokio::task::spawn_blocking(move || {
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
               return Err(error.into());
            }
         }
         Ok(bytes)
      })
      .await?
   }

   /// Encodes an image to WebP asynchronously.
   pub async fn encode_webp_data(image: RgbaImage) -> netcanv::Result<Vec<u8>> {
      Ok(tokio::task::spawn_blocking(move || {
         let image = DynamicImage::ImageRgba8(image);
         let encoder = webp::Encoder::from_image(&image).unwrap();
         encoder.encode(Self::WEBP_QUALITY).to_owned()
      })
      .await?)
   }

   /// Encodes a network image asynchronously. This encodes PNG, as well as WebP if the PNG is too
   /// large, and returns both images.
   pub async fn encode_network_data(image: RgbaImage) -> netcanv::Result<ChunkImage> {
      let png = Self::encode_png_data(image.clone()).await?;
      let webp = if png.len() > Self::MAX_PNG_SIZE {
         Some(Self::encode_webp_data(image).await?)
      } else {
         None
      };
      Ok(ChunkImage { png, webp })
   }

   /// Decodes a PNG file into the given sub-chunk.
   pub fn decode_png_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      let decoder = PngDecoder::new(Cursor::new(data))?;
      if decoder.color_type() != ColorType::Rgba8 {
         log::warn!("received non-RGBA image data, ignoring");
         return Err(Error::NonRgbaChunkImage);
      }
      let mut image = RgbaImage::from_pixel(Self::SIZE.0, Self::SIZE.1, Rgba([0, 0, 0, 0]));
      decoder.read_image(&mut image)?;
      Ok(image)
   }

   /// Decodes a WebP file into the given sub-chunk.
   fn decode_webp_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      let decoder = webp::Decoder::new(data);
      let image = match decoder.decode() {
         Some(image) => image.to_image(),
         None => return Err(Error::InvalidChunkImageFormat),
      }
      .into_rgba8();
      Ok(image)
   }

   /// Decodes a PNG or WebP file into the given sub-chunk, depending on what's actually stored in
   /// `data`.
   pub fn decode_network_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      // Try WebP first.
      let image = Self::decode_webp_data(data).or_else(|_| Self::decode_png_data(data))?;
      if image.dimensions() != Self::SIZE {
         log::error!(
            "received chunk with invalid size. got: {:?}, expected {:?}",
            image.dimensions(),
            Self::SIZE
         );
         Err(Error::InvalidChunkImageSize)
      } else {
         Ok(image)
      }
   }

   /// Marks the chunk as dirty - that is, invalidates any cached PNG and WebP data,
   /// and marks it as unsaved.
   pub fn mark_dirty(&mut self) {
      self.image_cache = None;
      self.saved = false;
   }

   /// Marks the given sub-chunk within this master chunk as saved.
   pub fn mark_saved(&mut self) {
      self.saved = true;
   }

   /// Iterates through all pixels within the image and checks whether any pixels in the image are
   /// not transparent.
   pub fn image_is_empty(image: &RgbaImage) -> bool {
      image.iter().all(|x| *x == 0)
   }
}
