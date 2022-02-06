mod common;
mod error;
mod from_language;
mod translate_enum;

use error::Error;
use proc_macro2::TokenStream;
use syn::DeriveInput;

fn wrap_fallible(
   input: proc_macro::TokenStream,
   f: impl FnOnce(&DeriveInput) -> Result<TokenStream, Error>,
) -> proc_macro::TokenStream {
   let ast = match syn::parse(input) {
      Ok(ast) => ast,
      Err(error) => {
         return Error {
            text: error.to_string(),
            span: error.span(),
         }
         .emit()
         .into()
      }
   };
   match f(&ast) {
      Ok(ast) => ast.into(),
      Err(error) => error.emit().into(),
   }
}

#[proc_macro_derive(FromLanguage)]
pub fn derive_from_language(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
   wrap_fallible(input, from_language::implementation)
}

#[proc_macro_derive(TranslateEnum, attributes(prefix))]
pub fn derive_translate_enum(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
   wrap_fallible(input, translate_enum::implementation)
}
