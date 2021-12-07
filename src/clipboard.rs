//! Platform-agnostic clipboard handling.

use std::borrow::Cow;
use std::sync::Mutex;

use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use once_cell::sync::Lazy;

static CLIPBOARD: Lazy<Mutex<Option<Clipboard>>> = Lazy::new(|| Mutex::new(None));

/// Initializes the clipboard in a platform-specific way.
#[allow(unused)]
pub fn init() -> anyhow::Result<()> {
   let mut clipboard = CLIPBOARD.lock().unwrap();
   *clipboard = Some(Clipboard::new()?);
   Ok(())
}

/// Copies the provided string into the clipboard.
pub fn copy_string(string: String) -> anyhow::Result<()> {
   let mut clipboard = CLIPBOARD.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      clipboard.set_text(string)?;
   } else {
      anyhow::bail!("clipboard is not initialized");
   }
   Ok(())
}

/// Copies the provided image into the clipboard.
pub fn copy_image(image: RgbaImage) -> anyhow::Result<()> {
   let mut clipboard = CLIPBOARD.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      clipboard.set_image(ImageData {
         width: image.width() as usize,
         height: image.height() as usize,
         bytes: Cow::Borrowed(&image),
      })?;
   } else {
      anyhow::bail!("clipboard is not initialized");
   }
   Ok(())
}

/// Pastes the contents of the clipboard into a string.
pub fn paste_string() -> anyhow::Result<String> {
   let mut clipboard = CLIPBOARD.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      Ok(clipboard.get_text()?)
   } else {
      anyhow::bail!("clipboard is not initialized");
   }
}

pub fn paste_image() -> anyhow::Result<RgbaImage> {
   let mut clipboard = CLIPBOARD.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      let image = clipboard.get_image()?.to_owned_img();
      Ok(RgbaImage::from_vec(
         image.width as u32,
         image.height as u32,
         match image.bytes {
            Cow::Borrowed(_) => unreachable!("clipboard data must be owned at this point"),
            Cow::Owned(data) => data,
         },
      )
      .expect("failed to create clipboard image"))
   } else {
      anyhow::bail!("clipboard is not initialized");
   }
}
