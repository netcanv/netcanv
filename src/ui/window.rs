//! A stacking window manager.

use std::collections::HashMap;

use netcanv_renderer::paws::Vector;

use crate::token::Token;

use super::view::View;
use super::{Input, Ui};

/// A window.
struct Window {
   view: View,
   content: Box<dyn WindowContent>,
}

/// A window manager.
pub struct WindowManager {
   window_id: Token,
   windows: HashMap<WindowId, Window>,
   stack: Vec<WindowId>,
}

impl WindowManager {
   /// Creates a new window manager.
   pub fn new() -> Self {
      Self {
         window_id: Token::new(0),
         windows: HashMap::new(),
         stack: Vec::new(),
      }
   }

   /// Opens a new window in the manager, and returns a handle for modifying it.
   pub fn open_window<C>(&mut self, view: View, content: C) -> WindowId
   where
      C: WindowContent + 'static,
   {
      let id = WindowId(self.window_id.next());
      let content: Box<dyn WindowContent> = Box::new(content);
      self.windows.insert(id.duplicate(), Window { view, content });
      id
   }

   /// Closes an open window.
   pub fn close_window(&mut self, window: WindowId) {
      self.stack.retain(|f| f != &window);
      self.windows.remove(&window);
   }
}

/// A unique window ID.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct WindowId(usize);

impl WindowId {
   /// Duplicates the window ID.
   ///
   /// This function is for internal use only, as there should only ever be a single external
   /// handle to a window.
   fn duplicate(&self) -> Self {
      Self(self.0)
   }
}

/// Window content. Defines what's drawn on a window.
pub trait WindowContent {
   fn process(&mut self, args: WindowContentArgs);
}

/// Arguments passed to [`WindowContent::process`].
pub struct WindowContentArgs<'u, 'i, 'v, 'ht> {
   pub ui: &'u mut Ui,
   pub input: &'i mut Input,
   pub view: &'v View,
   /// The hit test result.
   ///
   /// This can be set to determine the type of area the mouse cursor is under.
   /// See [`HitTest`]'s documentation for more information.
   pub hit_test: &'ht mut HitTest,
}

/// The result of a window hit test.
///
/// The hit test is used to determine what certain parts of a window are; for instance, whether
/// an area of the window is responsible for dragging the window around, or whether an area
/// is the close button of the window.
pub enum HitTest {
   /// Window content. This is the default value of the hit test.
   Content,
   /// A draggable area, such as the title bar.
   Draggable,
   /// A button that can be clicked to close the window.
   CloseButton,
}

impl Default for HitTest {
   fn default() -> Self {
      Self::Content
   }
}
