//! Common utilities such as converting casings.

pub fn snake_case_to_kebab_case(s: &str) -> String {
   s.chars().map(|c| if c == '_' { '-' } else { c }).collect()
}

pub fn pascal_case_to_kebab_case(s: &str) -> String {
   let mut result = String::with_capacity(s.len());

   if s.is_empty() {
      return result;
   }

   result.extend(s.chars().next().unwrap().to_lowercase());

   for c in s.chars().skip(1) {
      if c.is_uppercase() {
         result.push('-');
         result.extend(c.to_lowercase());
      } else {
         result.push(c);
      }
   }
   result
}
