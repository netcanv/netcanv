use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Data, Expr, FieldValue, Fields, Member, Token, Type};

struct Error {
   text: String,
   span: Span,
}

impl Error {
   fn new(span: Span, text: &str) -> Self {
      Self {
         text: text.to_owned(),
         span,
      }
   }

   fn emit(&self) -> TokenStream {
      let text = Literal::string(&self.text);
      quote_spanned! {self.span =>
         compile_error!(#text);
      }
   }
}

#[proc_macro_derive(FromLanguage)]
pub fn derive_from_language(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
   match from_language_impl(&ast) {
      Ok(ast) => ast.into(),
      Err(error) => error.emit().into(),
   }
}

struct FieldPairs {
   pairs: Vec<(Ident, Type)>,
}

fn from_language_impl(ast: &syn::DeriveInput) -> Result<TokenStream, Error> {
   if let Data::Struct(s) = &ast.data {
      if !matches!(s.fields, Fields::Named(_)) {
         return Err(Error::new(s.fields.span(), "struct must have named fields"));
      }
      let mut fields = FieldPairs { pairs: Vec::new() };
      for field in s.fields.iter() {
         let name = field.ident.as_ref().unwrap();
         if name.to_string().starts_with("r#") {
            return Err(Error::new(
               name.span(),
               "raw identifiers are not valid names",
            ));
         }
         fields.pairs.push((name.clone(), field.ty.clone()));
      }
      implement_trait(&ast.ident, &fields)
   } else {
      Err(Error::new(ast.span(), "struct expected"))
   }
}

fn implement_trait(typ: &Ident, fields: &FieldPairs) -> Result<TokenStream, Error> {
   let mut ctor_fields = Punctuated::<FieldValue, Comma>::new();
   let language = Ident::new("language", Span::call_site());
   let key = Ident::new("key", Span::call_site());
   let prefixed_key = Ident::new("prefixed_key", Span::call_site());

   for (name, typ) in &fields.pairs {
      let name_string = name.to_string().replace('_', "-");
      let name_string = Literal::string(&name_string);
      let value = quote!(
         <#typ as ::netcanv_i18n::from_language::FromLanguageKey>::from_language_key(
            #language,
            &format!("{}{}", #prefixed_key, #name_string),
         )
      );
      ctor_fields.push(FieldValue {
         attrs: Vec::new(),
         member: Member::Named(name.clone()),
         colon_token: Some(Token!(:)(Span::call_site())),
         expr: Expr::Verbatim(value),
      });
   }

   Ok(quote! {
      impl ::netcanv_i18n::from_language::FromLanguageKey for #typ {
         fn from_language_key(#language: &::netcanv_i18n::Language, #key: &str) -> Self {
            let #prefixed_key = if #key.is_empty() {
               "".to_owned()
            } else {
               format!("{}.", #key)
            };
            #typ { #ctor_fields }
         }
      }
   })
}
