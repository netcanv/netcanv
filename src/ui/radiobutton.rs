//! Radio buttons - sets of buttons with some data attached, of which only one can be active
//! at a given time.

use netcanv_renderer::paws::Layout;
use strum::{EnumMessage, IntoEnumIterator};

use crate::backend::Font;

use super::{Button, ButtonArgs, ButtonColors, Input, Ui};

/// The color scheme of a radio button.
#[derive(Clone)]
pub struct RadioButtonColors {
   pub normal: ButtonColors,
   pub selected: ButtonColors,
}

/// Arguments for processing a radio button.
#[derive(Clone)]
pub struct RadioButtonArgs<'c> {
   pub height: f32,
   pub colors: &'c RadioButtonColors,
   pub corner_radius: f32,
}

/// A radio button, whose currently selected item is one of `C`'s variants.
pub struct RadioButton<C>
where
   C: IntoEnumIterator + PartialEq,
{
   selected: C,
}

impl<C> RadioButton<C>
where
   C: IntoEnumIterator + PartialEq,
{
   /// Creates a new radio button, with the given item selected.
   pub fn new(selected: C) -> Self {
      Self { selected }
   }

   /// Processes the radio button, using `EnumMessage` to get the text of each variant.
   pub fn with_text(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      RadioButtonArgs {
         height,
         colors,
         corner_radius,
      }: RadioButtonArgs,
      font: &Font,
   ) -> RadioButtonProcessResult
   where
      C: EnumMessage,
   {
      let mut process_result = RadioButtonProcessResult { changed: false };

      ui.push((0.0, height), Layout::Horizontal);

      for item in C::iter() {
         if Button::with_text(
            ui,
            input,
            ButtonArgs {
               height,
               colors: if self.selected == item {
                  &colors.selected
               } else {
                  &colors.normal
               },
               corner_radius,
            },
            font,
            item.get_message().expect("one of the enum variants did not have a message"),
         )
         .clicked()
         {
            self.selected = item;
            process_result.changed = true;
         }
         ui.space(4.0);
      }

      ui.fit();
      ui.pop();

      process_result
   }

   /// Returns the selected variant.
   pub fn selected(&self) -> &C {
      &self.selected
   }
}

/// The result of processing a radio button.
pub struct RadioButtonProcessResult {
   changed: bool,
}

impl RadioButtonProcessResult {
   /// Returns whether the value was changed.
   pub fn changed(&self) -> bool {
      self.changed
   }
}
