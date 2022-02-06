//! The `TranslateEnum` trait.

use crate::Language;

pub trait TranslateEnum {
   /// Translates the enum to the given language.
   fn translate(&self, language: &Language) -> String;
}
