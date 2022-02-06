use netcanv_i18n::{Formatted, Language};

/// An error.
pub enum Error {
   CouldNotInitializeBackend { error: String },
   CouldNotInitializeLogger { error: String },
   TranslationsDoNotExist { language: String },
   CouldNotLoadLanguage { language: String },
}

impl Error {
   /// Translates the error into the given language.
   pub fn tr(&self, language: &Language) -> String {
      match self {
         Error::CouldNotInitializeBackend { error } => {
            Formatted::new(language.clone(), "error-could-not-initialize-backend")
               .format()
               .with("error", error.as_str())
               .done()
         }
         Error::CouldNotInitializeLogger { error } => {
            Formatted::new(language.clone(), "error-could-not-initialize-logger")
               .format()
               .with("error", error.as_str())
               .done()
         }
         Error::TranslationsDoNotExist {
            language: language_code,
         } => Formatted::new(language.clone(), "error-translations-do-not-exist")
            .format()
            .with("language", language_code.as_str())
            .done(),
         Error::CouldNotLoadLanguage {
            language: language_code,
         } => Formatted::new(language.clone(), "error-could-not-load-language")
            .format()
            .with("language", language_code.as_str())
            .done(),
      }
   }
}

pub type StdResult<T, E> = std::result::Result<T, E>;

pub type Result<T> = StdResult<T, Error>;
