use crate::assets::Assets;
use crate::backend::Image;

use super::Tool;

pub struct Selection {
   icon: Image,
}

impl Selection {
   pub fn new() -> Self {
      Self {
         icon: Assets::load_icon(include_bytes!("../../../assets/icons/selection.svg")),
      }
   }
}

impl Tool for Selection {
   fn name(&self) -> &str {
      "Selection"
   }

   fn icon(&self) -> &Image {
      &self.icon
   }
}
