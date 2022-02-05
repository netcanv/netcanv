//! Dynamic key lookup functionality.

use std::marker::PhantomData;

use crate::from_language::FromLanguageKey;
use crate::Language;

/// Dynamic map from keys to values.
///
/// A map has an associated prefix, eg. `tool`, and any value looked up from it is going to be
/// appended to that prefix, eg. `map.get("brush")` will result in `tool-brush` being looked up.
///
/// When `#[derive(FromLanguage)]` is used, the prefix is taken from the name of the field.
pub struct Map<T>
where
   T: FromLanguageKey,
{
   language: Language,
   prefix: String,
   _phantom_data: PhantomData<T>,
}

impl<T> Map<T>
where
   T: FromLanguageKey,
{
   /// Creates a new map. This is usually done by `#[derive(FromLanguage)]`.
   pub fn new(language: Language, prefix: &str) -> Self {
      Self {
         language,
         prefix: format!("{}-", prefix),
         _phantom_data: PhantomData,
      }
   }

   /// Returns the message for the given key.
   pub fn get(&self, key: &str) -> T {
      let key = format!("{}{}", self.prefix, key);
      T::from_language_key(&self.language, &key)
   }
}
