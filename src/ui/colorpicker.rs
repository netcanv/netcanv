//! Color picker with palettes.

use netcanv_renderer::paws::{point, Color, Layout, Rect, Renderer};
use netcanv_renderer_opengl::winit::event::MouseButton;

use crate::color::{AnyColor, Srgb};

use super::{Input, Ui, UiInput};

pub struct ColorPicker {
   palette: [AnyColor; Self::NUM_COLORS],
   color: AnyColor,
}

impl ColorPicker {
   /// The number of colors in a palette.
   const NUM_COLORS: usize = 9;

   /// Creates a new color picker.
   pub fn new() -> Self {
      let palette = [
         0x100820, // black
         0xff003e, // red
         0xff7b00, // orange
         0xffff00, // yellow
         0x2dd70e, // green
         0x03cbfb, // aqua
         0x0868eb, // blue
         0xa315d7, // purple
         0xffffff, // white
      ]
      .map(|hex| Srgb::from_color(Color::rgb(hex)).into());
      Self {
         palette,
         color: palette[0],
      }
   }

   /// Returns the (paws) color that's currently selected.
   pub fn color(&self) -> Color {
      Srgb::from(self.color).to_color(1.0)
   }

   /// Processes the color palette.
   pub fn process(&mut self, ui: &mut Ui, input: &Input) {
      for &color in &self.palette {
         ui.push((16.0, ui.height()), Layout::Freeform);
         let y_offset = ui.height()
            * if self.color == color {
               0.5
            } else if ui.has_mouse(&input) {
               0.7
            } else {
               0.8
            };
         let y_offset = y_offset.round();
         if ui.has_mouse(&input) && input.mouse_button_just_pressed(MouseButton::Left) {
            self.color = color;
         }
         ui.draw(|ui| {
            let rect = Rect::new(point(0.0, y_offset), ui.size());
            let color = Srgb::from(color).to_color(1.0);
            ui.render().fill(rect, color, 4.0);
         });
         ui.pop();
      }
   }
}
