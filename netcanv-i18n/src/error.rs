//! Error types.

use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
   InvalidLanguageCode,
}

impl Display for Error {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         Self::InvalidLanguageCode => write!(f, "invalid language code"),
      }
   }
}

impl std::error::Error for Error {}
