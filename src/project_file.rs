use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use image::{GenericImage, GenericImageView, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::backend::Backend;
use crate::image_coder::ImageCoder;
use crate::paint_canvas::chunk::Chunk;
use crate::paint_canvas::PaintCanvas;
use crate::Error;

/// The format version in a `.netcanv`'s `canvas.toml` file.
pub const CANVAS_TOML_VERSION: u32 = 1;

/// A `canvas.toml` file.
#[derive(Serialize, Deserialize)]
struct CanvasToml {
   /// The format version of the canvas.
   version: u32,
}

pub struct ProjectFile {
   /// The path to the `.netcanv` directory this paint canvas was saved to.
   filename: Option<PathBuf>,
}

impl ProjectFile {
   pub fn new() -> Self {
      ProjectFile { filename: None }
   }

   /// Saves the entire paint canvas to a PNG file.
   fn save_as_png(&self, path: &Path, canvas: &mut PaintCanvas) -> netcanv::Result<()> {
      log::info!("saving png {:?}", path);
      let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
      for chunk_position in canvas.chunks_mut().keys() {
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
      for (chunk_position, chunk) in canvas.chunks() {
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
            Chunk::SIZE.0,
            Chunk::SIZE.1,
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
   async fn save_as_netcanv(
      &mut self,
      path: &Path,
      canvas: &mut PaintCanvas,
   ) -> netcanv::Result<()> {
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
      for (chunk_position, chunk) in canvas.chunks_mut() {
         log::debug!("chunk {:?}", chunk_position);
         let image = chunk.download_image();
         let image_data = ImageCoder::encode_png_data(image).await?;
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
   pub fn save(&mut self, path: Option<&Path>, canvas: &mut PaintCanvas) -> netcanv::Result<()> {
      let path = path
         .map(|p| p.to_path_buf())
         .or_else(|| self.filename.clone())
         .expect("no save path provided");
      if let Some(ext) = path.extension() {
         match ext.to_str() {
            Some("png") => self.save_as_png(&path, canvas),
            Some("netcanv") | Some("toml") => {
               // TODO: Saving should be asynchronous.
               tokio::runtime::Handle::current()
                  .block_on(async { self.save_as_netcanv(&path, canvas).await })
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
   fn load_from_image_file(
      &mut self,
      renderer: &mut Backend,
      path: &Path,
      canvas: &mut PaintCanvas,
   ) -> netcanv::Result<()> {
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
            let chunk = canvas.ensure_chunk(renderer, offset_chunk_position);
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
            chunk_image.copy_from(&*sub_image, 0, 0)?;
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
   fn load_from_netcanv(
      &mut self,
      renderer: &mut Backend,
      path: &Path,
      canvas: &mut PaintCanvas,
   ) -> netcanv::Result<()> {
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
                  let chunk = canvas.ensure_chunk(renderer, chunk_position);
                  let image_data = ImageCoder::decode_png_data(&std::fs::read(path)?)?;
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
   pub fn load(
      &mut self,
      renderer: &mut Backend,
      path: &Path,
      canvas: &mut PaintCanvas,
   ) -> netcanv::Result<()> {
      if let Some(ext) = path.extension() {
         match ext.to_str() {
            Some("netcanv") | Some("toml") => self.load_from_netcanv(renderer, path, canvas),
            _ => self.load_from_image_file(renderer, path, canvas),
         }
      } else {
         self.load_from_image_file(renderer, path, canvas)
      }
   }

   /// Returns what filename the canvas was saved under.
   pub fn filename(&self) -> Option<&Path> {
      self.filename.as_deref()
   }
}
