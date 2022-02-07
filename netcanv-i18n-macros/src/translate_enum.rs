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

   let variants = if let Data::Enum(e) = &ast.data {
      collect_variants(&e.variants)?
   } else {
      return Err(Error::new(ast.span(), "enum expected"));
   };
   Ok(implement_trait(
      prefix.as_deref(),
      ast.ident.clone(),
      variants,
   ))
}

enum VariantFields {
   Nested(Box<Type>),
   Formatted(Vec<(Ident, Type)>),
}

struct Variant {
   name: Ident,
   fields: Option<VariantFields>,
}

fn collect_variants(ast: &Punctuated<syn::Variant, Comma>) -> Result<Vec<Variant>, Error> {
   let mut variants = Vec::new();
   for variant in ast {
      let fields = match &variant.fields {
         Fields::Unit => None,
         Fields::Unnamed(fields) => {
            if fields.unnamed.len() != 1 {
               return Err(Error::new(
                  fields.span(),
                  "one unnamed field expected to define attribute enum",
               ));
            }
            Some(VariantFields::Nested(Box::new(
               fields.unnamed[0].ty.clone(),
            )))
         }
         Fields::Named(fields) => {
            let mut f = Vec::new();
            for field in &fields.named {
               f.push((field.ident.as_ref().cloned().unwrap(), field.ty.clone()))
            }
            Some(VariantFields::Formatted(f))
         }
      };
      variants.push(Variant {
         name: variant.ident.clone(),
         fields,
      })
   }
   Ok(variants)
}

fn implement_trait(prefix: Option<&str>, typ: Ident, variants: Vec<Variant>) -> TokenStream {
   let language = Ident::new("__language", Span::call_site());
   let message = Ident::new("__message", Span::call_site());
   let mut arms = TokenStream::new();

   for Variant { name, fields } in variants {
      let variant_name = Literal::string(&if let Some(prefix) = prefix {
         format!(
            "{}-{}",
            prefix,
            pascal_case_to_kebab_case(&name.to_string())
         )
      } else {
         pascal_case_to_kebab_case(&name.to_string())
      });
      match fields {
         Some(VariantFields::Nested(inner)) => {
            let arm = quote! {
               Self::#name(__0) => {
                  assert!(#message.is_none(), "messages may only nest once");
                  <#inner as ::netcanv_i18n::translate_enum::TranslateEnumAttribute>::translate_attribute(__0, #language, Some(#variant_name))
               }
            };
            arms.extend(arm);
         }
         Some(VariantFields::Formatted(fields)) => {
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
               let field_name = Literal::string(&snake_case_to_kebab_case(&name.to_string()));
               with_chain.extend(quote! {
                  .with(#field_name, <#typ as ::std::clone::Clone>::clone(#renamed))
               });
            }
            let arm = quote! {
               Self::#name { #fields_ts } => {
                  if let Some(__parent_message) = #message {
                     Formatted::new(#language.clone(), format!(concat!("{}.", #variant_name), __parent_message))
                        .format()
                        #with_chain
                        .done()
                  } else {
                     Formatted::new(#language.clone(), #variant_name)
                        .format()
                        #with_chain
                        .done()
                  }
               },
            };
            arms.extend(arm);
         }
         None => {
            let arm = quote! {
               Self::#name => {
                  if let Some(__parent_message) = #message {
                     #language.message(&format!(concat!("{}.", #variant_name), __parent_message))
                  } else {
                     #language.message(#variant_name)
                  }
               },
            };
            arms.extend(arm);
         }
      }
   }

   quote! {
      impl ::netcanv_i18n::translate_enum::TranslateEnumAttribute for #typ {
         fn translate_attribute(
            &self,
            #language: &::netcanv_i18n::Language,
            #message: ::std::option::Option<&str>,
         ) -> ::std::string::String {
            match self {
               #arms
            }
         }
      }
   }
}
