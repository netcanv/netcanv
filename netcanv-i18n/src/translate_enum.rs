//! The `TranslateEnum` trait.

use crate::Language;

pub trait TranslateEnumAttribute {
   /// Translates the enum to the given language, with the given parent key.
   ///
   /// If the parent key is present, it should be prepended before the actual key that will be
   /// looked up, with a dot `.` after the parent key, such that this enum is looked up as an
   /// attribute of the parent message.
   fn translate_attribute(&self, language: &Language, message: Option<&str>) -> String;
}

/// Translating enums. This is implemented automatically for all enums that implement
/// `TranslateEnumAttribute`.
pub trait TranslateEnum {
   /// Translates the enum to the given language.
   fn translate(&self, language: &Language) -> String;
}

impl<T> TranslateEnum for T
where
   T: TranslateEnumAttribute,
{
   fn translate(&self, language: &Language) -> String {
      self.translate_attribute(language, None)
   }
}
