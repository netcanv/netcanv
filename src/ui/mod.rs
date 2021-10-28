//! UI controls.

use crate::backend::{Backend, Image};

mod button;
mod expand;
mod input;
mod slider;
mod textfield;

pub use button::*;
pub use expand::*;
pub use input::*;
use paws::{AlignH, Color, Point, Vector};
pub use slider::*;
pub use textfield::*;

pub type Ui = paws::Ui<Backend>;

pub trait UiInput {
   fn mouse_position(&self, input: &Input) -> Point;
   fn has_mouse(&self, input: &Input) -> bool;
}

impl UiInput for Ui {
   fn mouse_position(&self, input: &Input) -> Point {
      input.mouse_position() - self.position()
   }

   fn has_mouse(&self, input: &Input) -> bool {
      let mouse = self.mouse_position(input);
      println!("{:?}", mouse);
      let Vector {
         x: width,
         y: height,
      } = self.size();
      mouse.x >= 0.0 && mouse.x <= width && mouse.y >= 0.0 && mouse.y <= height
   }
}

pub trait UiElements {
   fn icon(&mut self, image: &Image, color: Color, size: Option<Vector>);
   fn paragraph(
      &mut self,
      color: Color,
      alignment: AlignH,
      line_spacing: Option<f32>,
      text: &[&str],
   );
}

impl UiElements for Ui {
   fn icon(&mut self, image: &Image, color: Color, size: Option<Vector>) {
      // TODO
   }

   fn paragraph(
      &mut self,
      color: Color,
      alignment: AlignH,
      line_spacing: Option<f32>,
      text: &[&str],
   ) {
      // TODO
   }
}

/// A trait implemented by elements that can be (un)focused.
pub trait Focus {
   fn focused(&self) -> bool;
   fn set_focus(&mut self, focused: bool);
}

/// Creates a _focus chain_, that is, a list of elements that can be `Tab`bed between.
pub fn chain_focus(input: &Input, fields: &mut [&mut dyn Focus]) {
   if input.key_just_typed(VirtualKeyCode::Tab) {
      macro_rules! process_focus_change {
         ($had_focus: expr, $element: expr) => {
            if $had_focus {
               $element.set_focus(true);
               return;
            }
            if $element.focused() {
               $element.set_focus(false);
               $had_focus = true;
            }
         };
      }

      let mut had_focus = false;

      if input.key_is_down(VirtualKeyCode::LShift) || input.key_is_down(VirtualKeyCode::RShift) {
         for element in fields.iter_mut().rev() {
            process_focus_change!(had_focus, element);
         }

         if !fields.is_empty() {
            fields[fields.len() - 1].set_focus(true);
         }
      } else {
         for element in fields.iter_mut() {
            process_focus_change!(had_focus, element);
         }

         if !fields.is_empty() {
            fields[0].set_focus(true);
         }
      }
   }
}
