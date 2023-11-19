use std::io::Cursor;

use ::image::codecs::png::{PngDecoder, PngEncoder};
use ::image::codecs::webp::{WebPDecoder, WebPEncoder, WebPQuality};
use ::image::{ColorType, ImageDecoder, Rgba, RgbaImage};
use image::{DynamicImage, ImageEncoder};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::backend::Backend;
use crate::paint_canvas::cache_layer::CachedChunk;
use crate::paint_canvas::chunk::Chunk;
use crate::Error;

pub struct ImageCoderChannels {
   pub decoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), RgbaImage)>,
   pub encoded_chunks_rx: mpsc::UnboundedReceiver<((i32, i32), CachedChunk)>,
}

pub struct ImageCoder {
   decoder_quitter: Option<(oneshot::Sender<()>, JoinHandle<()>)>,

   chunks_to_decode_tx: mpsc::UnboundedSender<((i32, i32), Vec<u8>)>,
   encoded_chunks_tx: mpsc::UnboundedSender<((i32, i32), CachedChunk)>,
}

impl ImageCoder {
   /// The maximum size threshold for a PNG to get converted to lossy WebP before network
   /// transmission.
   const MAX_PNG_SIZE: usize = 32 * 1024;

   pub fn new() -> (Self, ImageCoderChannels) {
      let (chunks_to_decode_tx, chunks_to_decode_rx) = mpsc::unbounded_channel();
      let (decoded_chunks_tx, decoded_chunks_rx) = mpsc::unbounded_channel();
      let (encoded_chunks_tx, encoded_chunks_rx) = mpsc::unbounded_channel();
      let (decoder_quit_tx, decoder_quit_rx) = oneshot::channel();

      let decode_join_handle = tokio::spawn({
         async move {
            ImageCoder::chunk_decoding_loop(
               chunks_to_decode_rx,
               decoded_chunks_tx,
               decoder_quit_rx,
            )
            .await;
         }
      });

      (
         Self {
            decoder_quitter: Some((decoder_quit_tx, decode_join_handle)),

            chunks_to_decode_tx,

            encoded_chunks_tx,
         },
         ImageCoderChannels {
            decoded_chunks_rx,
            encoded_chunks_rx,
         },
      )
   }

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
   async fn encode_network_data(image: RgbaImage) -> netcanv::Result<CachedChunk> {
      let png = Self::encode_png_data(image.clone()).await?;
      let webp = if png.len() > Self::MAX_PNG_SIZE {
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
   fn decode_network_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
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

   /// The decoding supervisor thread.
   async fn chunk_decoding_loop(
      mut input: mpsc::UnboundedReceiver<((i32, i32), Vec<u8>)>,
      output: mpsc::UnboundedSender<((i32, i32), RgbaImage)>,
      mut quit: oneshot::Receiver<()>,
   ) {
      tracing::info!("starting chunk decoding supervisor thread");
      loop {
         tokio::select! {
            biased;
            Ok(_) = &mut quit => break,
            data = input.recv() => {
               if let Some((chunk_position, image_data)) = data {
                  let output = output.clone();
                  tokio::task::spawn_blocking(move || match ImageCoder::decode_network_data(&image_data) {
                     Ok(image) => {
                        // Doesn't matter if the receiving half is closed.
                        let _ = output.send((chunk_position, image));
                     }
                     Err(error) => tracing::error!("image decoding failed: {:?}", error),
                  });
               } else {
                  tracing::info!("decoding supervisor: chunk data sender was dropped, quitting");
                  break;
               }
            },
         }
      }
      tracing::info!("exiting chunk decoding supervisor thread");
   }

   pub fn enqueue_chunk_encoding(
      &self,
      renderer: &mut Backend,
      chunk: &Chunk,
      output_channel: mpsc::UnboundedSender<((i32, i32), CachedChunk)>,
      chunk_position: (i32, i32),
   ) {
      // If the chunk's image is empty, there's no point in sending it.
      let image = chunk.download_image(renderer);
      if Chunk::image_is_empty(&image) {
         return;
      }
      // Otherwise, we can start encoding the chunk image.
      let encoded_chunks_tx = self.encoded_chunks_tx.clone();

      tokio::spawn(async move {
         tracing::debug!("encoding image data for chunk {:?}", chunk_position);
         let image_data = ImageCoder::encode_network_data(image).await;
         tracing::debug!("encoding done for chunk {:?}", chunk_position);
         match image_data {
            Ok(data) => {
               tracing::debug!("sending image data back to main thread");
               let _ = encoded_chunks_tx.send((chunk_position, data.clone()));
               let _ = output_channel.send((chunk_position, data));
            }
            Err(error) => {
               tracing::error!(
                  "error while encoding image for chunk {:?}: {:?}",
                  chunk_position,
                  error
               );
            }
         }
      });
   }

   pub fn enqueue_chunk_decoding(&self, to_chunk: (i32, i32), data: Vec<u8>) {
      self
         .chunks_to_decode_tx
         .send((to_chunk, data))
         .expect("Decoding supervisor thread should never quit");
   }

   pub fn send_encoded_chunk(
      &self,
      chunk: &CachedChunk,
      output_channel: mpsc::UnboundedSender<((i32, i32), CachedChunk)>,
      position: (i32, i32),
   ) {
      let _ = self.encoded_chunks_tx.send((position, chunk.to_owned()));
      let _ = output_channel.send((position, chunk.to_owned()));
   }

   pub async fn shutdown(mut self) {
      let (channel, join_handle) = self.decoder_quitter.take().unwrap();
      let _ = channel.send(());
      let _ = join_handle.await;
   }
}
