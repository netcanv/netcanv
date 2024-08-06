//! Platform-agnostic clipboard handling.

use std::borrow::Cow;
use std::sync::Mutex;

use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use once_cell::sync::Lazy;

use crate::Error;

struct ClipboardState {
   string: Mutex<Option<String>>,
   clipboard: Mutex<Option<Clipboard>>,
}

impl ClipboardState {
   fn new() -> Self {
      Self {
         string: Mutex::new(None),
         clipboard: Mutex::new(None),
      }
   }
}

static CLIPBOARD_STATE: Lazy<ClipboardState> = Lazy::new(ClipboardState::new);

/// Initializes the clipboard in a platform-specific way.
#[allow(unused)]
pub fn init() -> netcanv::Result<()> {
   profiling::scope!("clipboard::init");

   let mut clipboard = CLIPBOARD_STATE.clipboard.lock().unwrap();
   *clipboard = Some(Clipboard::new()?);
   Ok(())
}

/// Copies the provided string into the clipboard.
pub fn copy_string(string: String) -> netcanv::Result<()> {
   let mut clipboard = CLIPBOARD_STATE.clipboard.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      clipboard.set_text(string).map_err(|e| Error::CannotSaveToClipboard {
         error: e.to_string(),
      })?;
      Ok(())
   } else {
      Err(Error::ClipboardWasNotInitialized)
   }
}

/// Copies the provided string into the clipboard asynchronously, by updating given string
/// in internal state and then copying it on separate thread.
pub async fn copy_string_async(string: String) -> netcanv::Result<()> {
   // Update string to copy
   {
      let mut state_string = CLIPBOARD_STATE.string.lock().unwrap();
      *state_string = Some(string);
   }

   tokio::task::spawn_blocking(move || {
      // Wait for clipboard's mutex to unlock first, because by the time the clipboard unlocks,
      // string that we have to copy could have changed already.
      let mut clipboard = CLIPBOARD_STATE.clipboard.lock().unwrap();
      let mut string = CLIPBOARD_STATE.string.lock().unwrap();

      if let Some(clipboard) = &mut *clipboard {
         if let Some(string_to_copy) = (*string).take() {
            tracing::trace!("copying string into the clipboard");
            clipboard.set_text(&string_to_copy).map_err(|e| Error::CannotSaveToClipboard {
               error: e.to_string(),
            })?;
            tracing::trace!("string copied");
            Ok(())
         } else {
            Ok(())
         }
      } else {
         Err(Error::ClipboardWasNotInitialized)
      }
   })
   .await?
}

/// Copies the provided image into the clipboard.
pub fn copy_image(image: RgbaImage) -> netcanv::Result<()> {
   let mut clipboard = CLIPBOARD_STATE.clipboard.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      clipboard
         .set_image(ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Borrowed(&image),
         })
         .map_err(|e| Error::CannotSaveToClipboard {
            error: e.to_string(),
         })?;
      Ok(())
   } else {
      Err(Error::ClipboardWasNotInitialized)
   }
}

/// Pastes the contents of the clipboard into a string.
pub fn paste_string() -> netcanv::Result<String> {
   let mut clipboard = CLIPBOARD_STATE.clipboard.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      Ok(clipboard.get_text().map_err(|e| {
         if let arboard::Error::ContentNotAvailable = e {
            Error::ClipboardDoesNotContainText
         } else {
            e.into()
         }
      })?)
   } else {
      Err(Error::ClipboardWasNotInitialized)
   }
}

pub fn paste_image() -> netcanv::Result<RgbaImage> {
   let mut clipboard = CLIPBOARD_STATE.clipboard.lock().unwrap();
   if let Some(clipboard) = &mut *clipboard {
      let image = clipboard
         .get_image()
         .map_err(|e| {
            if let arboard::Error::ContentNotAvailable = e {
               Error::ClipboardDoesNotContainAnImage
            } else {
               e.into()
            }
         })?
         .to_owned_img();
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
      Err(Error::ClipboardWasNotInitialized)
   }
}
