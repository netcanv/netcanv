use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Data, DeriveInput, Fields, Lit, Meta, Type};

use crate::common::{pascal_case_to_kebab_case, snake_case_to_kebab_case};
use crate::error::Error;

pub(crate) fn implementation(ast: &DeriveInput) -> Result<TokenStream, Error> {
   let mut prefix = None;
   for attr in &ast.attrs {
      let meta = attr.parse_meta().map_err(|e| Error::new(e.span(), &e.to_string()))?;
      if let Some(ident) = meta.path().get_ident() {
         if *ident == "prefix" {
            if let Meta::NameValue(meta) = meta {
               if let Lit::Str(s) = meta.lit {
                  prefix = Some(s.value());
               }
            }
         }
      }
   }
   if prefix.is_none() {
      return Err(Error::new(
         ast.span(),
         "missing or invalid #[prefix = \"name\"] attribute",
      ));
   }

   let variants = if let Data::Enum(e) = &ast.data {
      collect_variants(&e.variants)?
   } else {
      return Err(Error::new(ast.span(), "enum expected"));
   };
   Ok(implement_trait(
      &prefix.unwrap(),
      ast.ident.clone(),
      variants,
   ))
}

struct Variant {
   name: Ident,
   fields: Option<Vec<(Ident, Type)>>,
}

fn collect_variants(ast: &Punctuated<syn::Variant, Comma>) -> Result<Vec<Variant>, Error> {
   let mut variants = Vec::new();
   for variant in ast {
      let fields = match &variant.fields {
         Fields::Named(fields) => {
            let mut f = Vec::new();
            for field in &fields.named {
               f.push((field.ident.as_ref().cloned().unwrap(), field.ty.clone()))
            }
            Some(f)
         }
         Fields::Unit => None,
         Fields::Unnamed(fields) => return Err(Error::new(fields.span(), "fields must be named")),
      };
      variants.push(Variant {
         name: variant.ident.clone(),
         fields,
      })
   }
   Ok(variants)
}

fn implement_trait(prefix: &str, typ: Ident, variants: Vec<Variant>) -> TokenStream {
   let language = Ident::new("__language", Span::call_site());
   let mut arms = TokenStream::new();

   for Variant { name, fields } in variants {
      let kebab_name = Literal::string(&format!(
         "{}-{}",
         prefix,
         pascal_case_to_kebab_case(&name.to_string())
      ));
      if let Some(fields) = fields {
         let mut fields_ts = TokenStream::new();
         let mut with_chain = TokenStream::new();
         let renamed_fields: Vec<_> = fields
            .iter()
            .enumerate()
            .map(|(i, (name, _))| Ident::new(&format!("__{}_{}", name, i), name.span()))
            .collect();
         for ((name, typ), renamed) in fields.iter().zip(&renamed_fields) {
            fields_ts.extend(quote! {
               #name: #renamed,
            });
            let kebab_name = Literal::string(&snake_case_to_kebab_case(&name.to_string()));
            with_chain.extend(quote! {
               .with(#kebab_name, <#typ as ::std::clone::Clone>::clone(#renamed))
            });
         }
         let arm = quote! {
            Self::#name { #fields_ts } => {
               Formatted::new(#language.clone(), #kebab_name)
                  .format()
                  #with_chain
                  .done()
            },
         };
         arms.extend(arm);
      } else {
         let arm = quote! {
            Self::#name => {
               #language.message(#kebab_name)
            },
         };
         arms.extend(arm);
      }
   }

   quote! {
      impl ::netcanv_i18n::translate_enum::TranslateEnum for #typ {
         fn translate(&self, #language: &::netcanv_i18n::Language) -> ::std::string::String {
            match self {
               #arms
            }
         }
      }
   }
}
