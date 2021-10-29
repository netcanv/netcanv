//! UI controls.

use netcanv_renderer::paws::{self, vector, AlignH, AlignV, Color, Layout, Point, Vector};
use netcanv_renderer::{Font as FontTrait, Image as ImageTrait, RenderBackend};

use crate::backend::{Backend, Font, Image};

mod button;
mod expand;
mod input;
mod slider;
mod textfield;

pub use button::*;
pub use expand::*;
pub use input::*;
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
      font: &Font,
      text: &[&str],
      color: Color,
      alignment: AlignH,
      line_spacing: Option<f32>,
   );
}

impl UiElements for Ui {
   fn icon(&mut self, image: &Image, color: Color, size: Option<Vector>) {
      let size = size.unwrap_or_else(|| vector(image.width() as f32, image.height() as f32));
      let icon = image.colorized(color);
      let position =
         self.position() + size / 2.0 - vector(image.width() as f32, image.height() as f32) / 2.0;
      self.push(size, Layout::Freeform);
      self.render().image(position, &icon);
      self.pop();
   }

   fn paragraph(
      &mut self,
      font: &Font,
      text: &[&str],
      color: Color,
      alignment: AlignH,
      line_spacing: Option<f32>,
   ) {
      let line_spacing = line_spacing.unwrap_or(1.2);
      let line_height = font.size() * line_spacing;
      let height = (line_height * text.len() as f32).round();
      self.push((self.width(), height), Layout::Vertical);
      for line in text {
         self.push((self.width(), line_height), Layout::Freeform);
         self.text(font, line, color, (alignment, AlignV::Middle));
         self.pop();
      }
      self.pop();
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
