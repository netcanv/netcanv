//! Language handling - loading of Fluent bundles and resources.

use std::fmt::Display;
use std::rc::Rc;

use fluent::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

use crate::Error;

#[derive(Clone)]
pub struct Language {
   pub(crate) bundle: Rc<FluentBundle<FluentResource>>,
}

impl Language {
   /// Loads a language with the given locale code, from the provided FTL source.
   pub fn load(language_code: &str, ftl_source: &str) -> Result<Self, Error> {
      log::info!("loading language {}", language_code);

      let identifier: LanguageIdentifier =
         language_code.parse().map_err(|_| Error::InvalidLanguageCode)?;
      let mut bundle = FluentBundle::new(vec![identifier]);
      let resource = match FluentResource::try_new(ftl_source.to_owned()) {
         Ok(resource) => resource,
         Err((resource, errors)) => {
            Self::log_errors(language_code, &errors);
            resource
         }
      };
      if let Err(errors) = bundle.add_resource(resource) {
         Self::log_errors(language_code, &errors);
      }

      Ok(Self {
         bundle: Rc::new(bundle),
      })
   }

   fn log_errors<T>(language_code: &str, errors: &[T])
   where
      T: Display,
   {
      if !errors.is_empty() {
         log::error!("errors occured in language {}", language_code);
         for error in errors {
            log::error!("{}", error);
         }
      }
   }

   /// Returns a non-parametric message.
   pub fn message(&self, key: &str) -> String {
      let mut errors = Vec::new();
      let message = match self.bundle.get_message(key) {
         Some(message) => message,
         None => return key.to_owned(),
      };
      let pattern = match message.value() {
         Some(value) => value,
         None => {
            log::error!("message with no value");
            return key.to_owned();
         }
      };
      self.bundle.format_pattern(pattern, None, &mut errors).into_owned()
   }
}
