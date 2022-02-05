//! Formatted messages.

use std::borrow::Cow;

use fluent::{FluentArgs, FluentValue};

use crate::Language;

/// A formatted message.
pub struct Formatted {
   key: Cow<'static, str>,
}

impl Formatted {
   /// Creates a new formatted message.
   pub fn new(key: impl Into<Cow<'static, str>>) -> Self {
      Self { key: key.into() }
   }

   /// Begins formatting a formatted message.
   pub fn format(&self) -> Formatter<'_> {
      Formatter {
         key: &self.key,
         args: FluentArgs::with_capacity(4),
      }
   }
}

/// A message formatter. Contains the set of arguments to format the message with.
pub struct Formatter<'f> {
   key: &'f str,
   args: FluentArgs<'f>,
}

impl<'f> Formatter<'f> {
   /// Adds an argument to the formatter.
   pub fn with(mut self, key: &'static str, value: impl Into<FormatArg<'f>>) -> Self {
      self.args.set(key, value.into());
      self
   }

   /// Finishes formatting the string.
   pub fn done(self, language: &Language) -> String {
      let mut errors = Vec::new();
      let message = match language.bundle.get_message(self.key) {
         Some(message) => message,
         None => return self.key.to_owned(),
      };
      let pattern = match message.value() {
         Some(value) => value,
         None => {
            log::error!("message with no value");
            return self.key.to_owned();
         }
      };
      language.bundle.format_pattern(pattern, Some(&self.args), &mut errors).into_owned()
   }
}

/// Format arguments.
pub enum FormatArg<'a> {
   Signed(i64),
   Unsigned(u64),
   Float(f64),
   String(&'a str),
}

macro_rules! format_arg_from {
   ($from:ty, $variant:tt) => {
      impl From<$from> for FormatArg<'_> {
         fn from(x: $from) -> Self {
            Self::$variant(x as _)
         }
      }
   };
}

format_arg_from!(u8, Unsigned);
format_arg_from!(u16, Unsigned);
format_arg_from!(u32, Unsigned);
format_arg_from!(u64, Unsigned);
format_arg_from!(usize, Unsigned);

format_arg_from!(i8, Signed);
format_arg_from!(i16, Signed);
format_arg_from!(i32, Signed);
format_arg_from!(i64, Signed);
format_arg_from!(isize, Signed);

format_arg_from!(f32, Float);
format_arg_from!(f64, Float);

impl<'a> From<&'a str> for FormatArg<'a> {
   fn from(s: &'a str) -> Self {
      Self::String(s)
   }
}

impl<'a> From<FormatArg<'a>> for FluentValue<'a> {
   fn from(arg: FormatArg<'a>) -> Self {
      match arg {
         FormatArg::Signed(x) => x.into(),
         FormatArg::Unsigned(x) => x.into(),
         FormatArg::Float(x) => x.into(),
         FormatArg::String(s) => s.into(),
      }
   }
}
