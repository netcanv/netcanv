//! UI controls.

use netcanv_renderer::paws::{self, vector, AlignH, AlignV, Color, Layout, Point, Vector};
use netcanv_renderer::{Font as FontTrait, Image as ImageTrait, RenderBackend};

use crate::backend::winit::keyboard::{Key, NamedKey};
use crate::backend::{Backend, Font, Image};

mod button;
mod color_picker;
mod context_menu;
mod expand;
mod input;
mod radio_button;
mod slider;
mod text_field;
mod tooltip;
pub mod view;
pub mod wm;

pub use button::*;
pub use color_picker::*;
pub use context_menu::*;
pub use expand::*;
pub use input::*;
pub use radio_button::*;
pub use slider::*;
pub use text_field::*;
pub use tooltip::*;

pub type Ui = paws::Ui<Backend>;

pub trait UiInput {
   /// Returns the mouse position relative to the current group.
   fn mouse_position(&self, input: &Input) -> Point;

   /// Returns the previous mouse position relative to the current group.
   fn previous_mouse_position(&self, input: &Input) -> Point;

   /// Returns whether the current group contains the given point.
   fn has_point(&self, point: Point) -> bool;

   /// Returns whether the mouse position is in the current group's rectangle.
   fn has_mouse(&self, input: &Input) -> bool;

   /// Returns whether the mouse position is in the current group's rectangle, and the mouse
   /// is currently active.
   fn hover(&self, input: &Input) -> bool;

   /// Returns whether the current group has just been clicked with the given mouse button.
   fn clicked(&self, input: &Input, button: MouseButton) -> bool;
}

impl UiInput for Ui {
   fn mouse_position(&self, input: &Input) -> Point {
      input.mouse_position() - self.position()
   }

   fn previous_mouse_position(&self, input: &Input) -> Point {
      input.previous_mouse_position() - self.position()
   }

   fn has_point(&self, point: Point) -> bool {
      let Point { x, y } = self.position();
      let Vector {
         x: width,
         y: height,
      } = self.size();
      point.x >= x && point.x <= x + width && point.y >= y && point.y <= y + height
   }

   fn has_mouse(&self, input: &Input) -> bool {
      let mouse = input.mouse_position();
      self.has_point(mouse)
   }

   fn hover(&self, input: &Input) -> bool {
      input.mouse_active() && self.has_mouse(input)
   }

   fn clicked(&self, input: &Input, button: MouseButton) -> bool {
      input.mouse_button_just_released(button) && self.has_point(input.click_position(button))
   }
}

pub trait UiElements {
   /// Draws a colorized image centered in a new group.
   fn icon(&mut self, image: &Image, color: Color, size: Option<Vector>);

   /// Draws text in a new group.
   fn vertical_label(&mut self, font: &Font, text: &str, color: Color, alignment: AlignH);

   /// Draws text in a new group.
   ///
   /// Intended for use with horizontal layouts. Will not work all that well with vertical.
   /// Use [`UiElements::vertical_label`] instead.
   fn horizontal_label(
      &mut self,
      font: &Font,
      text: &str,
      color: Color,
      constraint: Option<(f32, AlignH)>,
   );

   /// Draws a paragraph of text. Each string in `text` is treated as a new group.
   fn paragraph<T, S>(
      &mut self,
      font: &Font,
      text: T,
      color: Color,
      alignment: AlignH,
      line_spacing: Option<f32>,
   ) where
      T: IntoIterator<Item = S>,
      S: AsRef<str>;
}

impl UiElements for Ui {
   fn icon(&mut self, image: &Image, color: Color, size: Option<Vector>) {
      let size = size.unwrap_or_else(|| vector(image.width() as f32, image.height() as f32));
      let icon = image.colorized(color);
      let position = size / 2.0 - vector(image.width() as f32, image.height() as f32) / 2.0;
      self.push(size, Layout::Freeform);
      self.draw(|ui| {
         ui.render().image(icon.rect(position), &icon);
      });
      self.pop();
   }

   fn vertical_label(&mut self, font: &Font, text: &str, color: Color, alignment: AlignH) {
      self.push((self.width(), font.height()), Layout::Freeform);
      self.text(font, text, color, (alignment, AlignV::Top));
      self.pop();
   }

   fn horizontal_label(
      &mut self,
      font: &Font,
      text: &str,
      color: Color,
      width: Option<(f32, AlignH)>,
   ) {
      let (width, alignment) = width.unwrap_or_else(|| (font.text_width(text), AlignH::Left));
      self.push((width, self.height()), Layout::Freeform);
      self.text(font, text, color, (alignment, AlignV::Middle));
      self.pop();
   }

   fn paragraph<T, S>(
      &mut self,
      font: &Font,
      text: T,
      color: Color,
      alignment: AlignH,
      line_spacing: Option<f32>,
   ) where
      T: IntoIterator<Item = S>,
      S: AsRef<str>,
   {
      let line_spacing = line_spacing.unwrap_or(1.2);
      let line_height = (font.size() * line_spacing).ceil();
      self.push((self.width(), 0.0), Layout::Vertical);
      for line in text.into_iter() {
         self.push((self.width(), line_height), Layout::Freeform);
         self.text(font, line.as_ref(), color, (alignment, AlignV::Middle));
         self.pop();
      }
      self.fit();
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
   macro_rules! process_focus_change {
      ($had_focus:expr, $element:expr) => {
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

   match input.action((Modifier::SHIFT, Key::Named(NamedKey::Tab))) {
      (true, true) => {
         for element in fields.iter_mut().rev() {
            process_focus_change!(had_focus, element);
         }
         if !fields.is_empty() {
            fields[fields.len() - 1].set_focus(true);
         }
      }
      (false, true) => {
         for element in fields.iter_mut() {
            process_focus_change!(had_focus, element);
         }
         if !fields.is_empty() {
            fields[0].set_focus(true);
         }
      }
      _ => (),
   }
}
