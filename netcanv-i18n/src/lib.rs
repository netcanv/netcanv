extern crate self as netcanv_i18n;

mod error;
mod format;
pub mod from_language;
mod language;
mod map;
pub mod translate_enum;

pub use error::*;
pub use format::{FormatArg, Formatted};
pub use language::*;
pub use map::Map;

pub use netcanv_i18n_macros::{FromLanguage, TranslateEnum};

pub use unic_langid;

#[allow(dead_code)]
fn _expand_translate_enum() {
   #[derive(TranslateEnum)]
   enum Nested {
      FirstError,
      SecondError,
   }

   #[derive(TranslateEnum)]
   #[prefix = "error"]
   enum Error {
      Test,
      MultipleWordsHelloWorld,
      WithFields { a_field: String },
      WithNested(Nested),
   }
}
