use std::num::{IntErrorKind, ParseIntError};

use image::ImageError;
use netcanv_i18n::{Formatted, TranslateEnum};
use netcanv_protocol::relay;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinError;
use tokio_tungstenite::tungstenite;

/// An error.
#[derive(Debug, TranslateEnum)]
#[prefix = "error"]
pub enum Error {
   //
   // Generic
   //
   Io { error: String },
   Image { error: String },
   Join { error: String },
   ChannelSend,
   TomlParse { error: String },
   TomlSerialization { error: String },
   InvalidUtf8,

   FailedToPersistTemporaryFile { error: String },

   //
   // Parsing
   //
   NumberIsEmpty,
   InvalidDigit,
   NumberTooBig,
   NumberTooSmall,
   NumberMustNotBeZero,
   // This variant is triggered by ParseIntErrors that were introduced in new versions of Rust and
   // were not yet added to the `match` in `From<ParseIntError>`.
   InvalidNumber,

   //
   // Initialization
   //
   CouldNotInitializeBackend { error: String },
   CouldNotInitializeLogger { error: String },

   //
   // Clipboard
   //
   ClipboardWasNotInitialized,
   CannotSaveToClipboard { error: String },
   ClipboardDoesNotContainText,
   ClipboardDoesNotContainAnImage,
   ClipboardContentUnavailable,
   ClipboardNotSupported,
   ClipboardOccupied,
   ClipboardConversion,
   ClipboardUnknown { error: String },

   //
   // User config
   //
   ConfigIsAlreadyLoaded,

   //
   // Translations
   //
   TranslationsDoNotExist { language: String },
   CouldNotLoadLanguage { language: String },

   //
   // License page
   //
   CouldNotOpenWebBrowser,
   NoLicensingInformationAvailable,

   //
   // Paint canvas
   //
   NonRgbaChunkImage,
   InvalidChunkImageFormat,
   InvalidChunkImageSize,
   NothingToSave,
   InvalidCanvasFolder,
   UnsupportedSaveFormat,
   MissingCanvasSaveExtension,
   InvalidChunkPositionPattern,
   TrailingChunkCoordinatesInFilename,
   CanvasTomlVersionMismatch,

   //
   // Socket networking
   //
   InvalidUrl,
   NoVersionPacket,
   InvalidVersionPacket,
   RelayIsTooOld,
   RelayIsTooNew,
   ReceivedPacketThatIsTooBig,
   TriedToSendPacketThatIsTooBig { max: usize, size: usize },
   TriedToSendPacketThatIsWayTooBig,
   RelayHasDisconnected,
   WebSocket { error: String },

   //
   // Peer networking
   //
   NotConnectedToRelay,
   NotConnectedToHost,
   PacketSerializationFailed { error: String },
   PacketDeserializationFailed { error: String },
   Relay(relay::Error),
   UnexpectedRelayPacket,
   ClientIsTooOld,
   ClientIsTooNew,

   //
   // Tools
   //
   InvalidToolPacket,
}

macro_rules! error_from {
   ($T:ty, $variant:path) => {
      impl From<$T> for Error {
         fn from(error: $T) -> Self {
            $variant {
               error: error.to_string(),
            }
         }
      }
   };
}

error_from!(std::io::Error, Error::Io);
error_from!(ImageError, Error::Image);
error_from!(JoinError, Error::Join);
error_from!(toml::de::Error, Error::TomlParse);
error_from!(toml::ser::Error, Error::TomlSerialization);
error_from!(tungstenite::Error, Error::WebSocket);

impl<T> From<mpsc::error::SendError<T>> for Error {
   fn from(_: mpsc::error::SendError<T>) -> Self {
      Self::ChannelSend
   }
}

impl<T> From<broadcast::error::SendError<T>> for Error {
   fn from(_: broadcast::error::SendError<T>) -> Self {
      Self::ChannelSend
   }
}

impl From<ParseIntError> for Error {
   fn from(error: ParseIntError) -> Self {
      match error.kind() {
         IntErrorKind::Empty => Self::NumberIsEmpty,
         IntErrorKind::InvalidDigit => Self::InvalidDigit,
         IntErrorKind::PosOverflow => Self::NumberTooBig,
         IntErrorKind::NegOverflow => Self::NumberTooSmall,
         IntErrorKind::Zero => Self::NumberMustNotBeZero,
         _ => Self::InvalidNumber,
      }
   }
}

impl From<arboard::Error> for Error {
   fn from(error: arboard::Error) -> Self {
      match error {
         arboard::Error::ContentNotAvailable => Self::ClipboardContentUnavailable,
         arboard::Error::ClipboardNotSupported => Self::ClipboardNotSupported,
         arboard::Error::ClipboardOccupied => Self::ClipboardOccupied,
         arboard::Error::ConversionFailure => Self::ClipboardConversion,
         arboard::Error::Unknown { description } => Self::ClipboardUnknown { error: description },
         _ => Self::ClipboardUnknown {
            error: error.to_string(),
         },
      }
   }
}

pub type StdResult<T, E> = std::result::Result<T, E>;

pub type Result<T> = StdResult<T, Error>;

#[macro_export]
macro_rules! ensure {
   ($cond:expr, $error:expr) => {
      #[allow(clippy::neg_cmp_op_on_partial_ord)]
      {
         if !($cond) {
            return Err($error);
         }
      }
   };
}
