use std::io::Cursor;

use ::image::codecs::png::{PngDecoder, PngEncoder};
use ::image::codecs::webp::{WebPDecoder, WebPEncoder, WebPQuality};
use ::image::{ColorType, ImageDecoder, Rgba, RgbaImage};
use image::{DynamicImage, ImageEncoder};

use crate::paint_canvas::cache_layer::CachedChunk;
use crate::paint_canvas::chunk::Chunk;
use crate::Error;

pub struct ImageCoder;

impl ImageCoder {
   /// The maximum size threshold for a PNG to get converted to lossy WebP before network
   /// transmission.
   const MAX_PNG_SIZE: usize = 32 * 1024;

   /// Encodes an image to PNG data asynchronously.
   pub async fn encode_png_data(image: RgbaImage) -> netcanv::Result<Vec<u8>> {
      tokio::task::spawn_blocking(move || {
         let mut bytes: Vec<u8> = Vec::new();
         match PngEncoder::new(Cursor::new(&mut bytes)).write_image(
            &image,
            image.width(),
            image.height(),
            ColorType::Rgba8,
         ) {
            Ok(()) => (),
            Err(error) => {
               tracing::error!("error while encoding: {}", error);
               return Err(error.into());
            }
         }
         Ok(bytes)
      })
      .await?
   }

   /// Encodes an image to WebP asynchronously.
   async fn encode_webp_data(image: RgbaImage) -> netcanv::Result<Vec<u8>> {
      tokio::task::spawn_blocking(move || {
         let mut bytes: Vec<u8> = Vec::new();
         match WebPEncoder::new_with_quality(
            Cursor::new(&mut bytes),
            WebPQuality::lossy(WebPQuality::DEFAULT),
         )
         .write_image(&image, image.width(), image.height(), ColorType::Rgba8)
         {
            Ok(()) => (),
            Err(error) => {
               tracing::error!("error while encoding: {}", error);
               return Err(error.into());
            }
         }
         Ok(bytes)
      })
      .await?
   }

   /// Encodes a network image asynchronously. This encodes PNG, as well as WebP if the PNG is too
   /// large, and returns both images.
   pub async fn encode_network_data(image: RgbaImage) -> netcanv::Result<CachedChunk> {
      let png = Self::encode_png_data(image.clone()).await?;
      let webp = if png.len() > Self::MAX_PNG_SIZE {
         tracing::debug!("webp");
         Some(Self::encode_webp_data(image).await?)
      } else {
         None
      };
      Ok(CachedChunk { png, webp })
   }

   /// Decodes a PNG file into the given sub-chunk.
   pub fn decode_png_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      let decoder = PngDecoder::new(Cursor::new(data))?;
      if decoder.color_type() != ColorType::Rgba8 {
         tracing::warn!("received non-RGBA image data, ignoring");
         return Err(Error::NonRgbaChunkImage);
      }
      let mut image = RgbaImage::from_pixel(Chunk::SIZE.0, Chunk::SIZE.1, Rgba([0, 0, 0, 0]));
      decoder.read_image(&mut image)?;
      Ok(image)
   }

   /// Decodes a WebP file into the given sub-chunk.
   fn decode_webp_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      let decoder = WebPDecoder::new(Cursor::new(data))?;
      let image = DynamicImage::from_decoder(decoder)?.into_rgba8();
      Ok(image)
   }

   /// Decodes a PNG or WebP file into the given sub-chunk, depending on what's actually stored in
   /// `data`.
   pub fn decode_network_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      // Try WebP first.
      let image = Self::decode_webp_data(data).or_else(|_| Self::decode_png_data(data))?;
      if image.dimensions() != Chunk::SIZE {
         tracing::error!(
            "received chunk with invalid size. got: {:?}, expected {:?}",
            image.dimensions(),
            Chunk::SIZE
         );
         Err(Error::InvalidChunkImageSize)
      } else {
         Ok(image)
      }
   }
}
