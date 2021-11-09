//! The Brush tool. Allows for painting, as well as erasing pixels from the canvas.

use netcanv_renderer::paws::Color;

use crate::assets::Assets;
use crate::backend::Image;

use super::Tool;

pub struct Brush {
   icon: Image,
}

impl Brush {
   pub fn new() -> Self {
      Self {
         icon: Assets::load_icon(include_bytes!("../../../assets/icons/brush.svg")),
      }
   }
}

impl Tool for Brush {
   fn name(&self) -> &str {
      "Brush"
   }

   fn icon(&self) -> &Image {
      &self.icon
   }
}

/// The palette of colors at the bottom of the screen.
const COLOR_PALETTE: &[Color] = &[
   Color::rgb(0x100820), // black
   Color::rgb(0xff003e), // red
   Color::rgb(0xff7b00), // orange
   Color::rgb(0xffff00), // yellow
   Color::rgb(0x2dd70e), // green
   Color::rgb(0x03cbfb), // aqua
   Color::rgb(0x0868eb), // blue
   Color::rgb(0xa315d7), // purple
   Color::rgb(0xffffff), // white
];
