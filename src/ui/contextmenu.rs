//! A context menu that can be opened and closed at will.
//!
//! The opening interaction is handled by external events.

use crate::backend::winit::event::MouseButton;
use netcanv_renderer::paws::{Color, Layout};

use super::view::{Dimensions, View};
use super::{Input, Ui};

/// The state for a context menu.
pub struct ContextMenu {
   pub view: View,
   is_open: bool,
   just_opened: bool,
}

/// The color scheme of a context menu.
#[derive(Clone)]
pub struct ContextMenuColors {
   pub background: Color,
}

/// The arguments passed to [`ContextMenu::begin`].
pub struct ContextMenuArgs<'c> {
   pub colors: &'c ContextMenuColors,
}

impl ContextMenu {
   /// Creates a new context menu with the given dimensions.
   pub fn new(dimensions: impl Into<Dimensions>) -> Self {
      Self {
         view: View::new(dimensions),
         is_open: false,
         just_opened: false,
      }
   }

   /// Opens the context menu.
   pub fn open(&mut self) {
      self.is_open = true;
      self.just_opened = true;
   }

   /// Closes the context menu.
   pub fn close(&mut self) {
      self.is_open = false;
   }

   /// Toggles the context menu open.
   pub fn toggle(&mut self) {
      if self.is_open {
         self.close();
      } else {
         self.open();
      }
   }

   /// Begins drawing to the context menu.
   ///
   /// This is usually used with an `if` statement, like so:
   /// ```
   /// if menu.begin(ui, input).is_open() {
   ///    // draw in the menu here
   ///
   ///    menu.end(ui);
   /// }
   /// ```
   pub fn begin(
      &mut self,
      ui: &mut Ui,
      input: &mut Input,
      args: ContextMenuArgs,
   ) -> ContextMenuBeginResult {
      // A bit of a hack to receive all mouse events.
      if !self.just_opened
         && input.mouse_button_just_released(MouseButton::Left)
         && !self.view.has_mouse(input)
      {
         self.is_open = false;
      }
      if self.is_open {
         self.view.begin(ui, input, Layout::Vertical);
         ui.fill_rounded(args.colors.background, 4.0);
      }
      self.just_opened = false;
      ContextMenuBeginResult {
         is_open: self.is_open,
      }
   }

   /// Finishes drawing inside the context menu.
   ///
   /// This should be called _inside_ the body of the `if` statement that called `begin()` in its
   /// condition.
   pub fn end(&mut self, ui: &mut Ui) {
      self.view.end(ui);
   }
}

pub struct ContextMenuBeginResult {
   is_open: bool,
}

impl ContextMenuBeginResult {
   /// Returns whether the context menu is currently open.
   pub fn is_open(self) -> bool {
      self.is_open
   }
}
