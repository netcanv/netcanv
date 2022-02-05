//! Language handling - loading of Fluent bundles and resources.

use std::fmt::Display;
use std::rc::Rc;

use fluent::{FluentBundle, FluentMessage, FluentResource};
use fluent_syntax::ast::Pattern;
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

   fn get_message(&self, key: &str) -> Option<FluentMessage> {
      Some(match self.bundle.get_message(key) {
         Some(message) => message,
         None => {
            log::error!("message {:?} is missing", key);
            return None;
         }
      })
   }

   /// Resolves the key to a pattern.
   ///
   /// Note that the key can contain a dot, in which case an attribute will be looked up from the
   /// message under the given key. For instance, `example.hello` will look up attribute `hello`
   /// from message `example`. An attribute can only appear once.
   pub(crate) fn get_pattern(&self, key: &str) -> Option<&Pattern<&str>> {
      if let Some(dot_index) = key.find('.') {
         let message_name = &key[..dot_index];
         let message = self.get_message(message_name)?;
         let attribute_name = &key[(dot_index + 1)..];
         let attribute = match message.get_attribute(attribute_name) {
            Some(attribute) => attribute,
            None => {
               log::error!(
                  "message {:?} does not have the attribute {:?}",
                  &key[dot_index..],
                  attribute_name
               );
               return None;
            }
         };
         Some(attribute.value())
      } else {
         let message = self.get_message(key)?;
         Some(match message.value() {
            Some(value) => value,
            None => {
               log::error!("message {:?} doesn't have a value", key);
               return None;
            }
         })
      }
   }

   /// Returns a non-parametric message.
   pub fn message(&self, key: &str) -> String {
      let mut errors = Vec::new();
      let pattern = match self.get_pattern(key) {
         Some(pattern) => pattern,
         None => return key.to_owned(),
      };
      self.bundle.format_pattern(pattern, None, &mut errors).into_owned()
   }
}
