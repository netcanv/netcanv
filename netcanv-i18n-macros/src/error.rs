use proc_macro2::{Literal, Span, TokenStream};
use quote::quote_spanned;

pub(crate) struct Error {
   pub(crate) text: String,
   pub(crate) span: Span,
}

impl Error {
   pub(crate) fn new(span: Span, text: &str) -> Self {
      Self {
         text: text.to_owned(),
         span,
      }
   }

   pub(crate) fn emit(&self) -> TokenStream {
      let text = Literal::string(&self.text);
      quote_spanned! {self.span =>
         compile_error!(#text);
      }
   }
}
