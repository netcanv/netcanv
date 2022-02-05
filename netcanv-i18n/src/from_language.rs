use crate::{Formatted, Language};

pub trait FromLanguage {
   /// Extracts `Self` from the language, given the key.
   fn from_language(language: &Language, key: &'static str) -> Self;
}

/// Extracts non-formatted strings from languages.
impl FromLanguage for String {
   fn from_language(language: &Language, key: &'static str) -> Self {
      language.message(key)
   }
}

/// Extracts formatted strings from languages.
impl FromLanguage for Formatted {
   fn from_language(_language: &Language, key: &'static str) -> Self {
      Self::new(key.to_owned())
   }
}
