use crate::{Formatted, Language, Map};

pub trait FromLanguageKey {
   /// Extracts `Self` from the language, given the key.
   fn from_language_key(language: &Language, key: &str) -> Self;
}

/// Extracts non-formatted strings from languages.
impl FromLanguageKey for String {
   fn from_language_key(language: &Language, key: &str) -> Self {
      language.message(key)
   }
}

/// Extracts formatted strings from languages.
impl FromLanguageKey for Formatted {
   fn from_language_key(language: &Language, key: &str) -> Self {
      Self::new(language.clone(), key.to_owned())
   }
}

impl<T> FromLanguageKey for Map<T>
where
   T: FromLanguageKey,
{
   fn from_language_key(language: &Language, key: &str) -> Self {
      Self::new(language.clone(), key)
   }
}

pub trait FromLanguage {
   /// Constructs `Self` by looking up strings from the given language.
   fn from_language(language: &Language) -> Self;
}

impl<T> FromLanguage for T
where
   T: FromLanguageKey,
{
   fn from_language(language: &Language) -> Self {
      Self::from_language_key(language, "")
   }
}
