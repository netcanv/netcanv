//! A stacking window manager.

pub mod windows;

use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::token::Token;

use super::view::View;
use super::{Input, Ui};

/// A window.
struct Window {
   view: View,
   content: Box<dyn UntypedWindowContent>,
   data: Box<dyn Any>,
}

/// A window manager.
pub struct WindowManager {
   window_id: Token,
   windows: HashMap<UntypedWindowId, Window>,
   stack: Vec<UntypedWindowId>,
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

   /// Returns a mutable reference to the given window's data.
   pub fn window_data_mut<D>(&mut self, id: &WindowId<D>) -> &mut D
   where
      D: Any,
   {
      self.windows.get_mut(&id.0).unwrap().data.downcast_mut().unwrap()
   }

   /// Opens a new window in the manager, and returns a handle for modifying it.
   pub fn open_window<C, D>(&mut self, view: View, content: C, data: D) -> WindowId<D>
   where
      C: WindowContent<Data = D> + 'static,
      D: Any,
   {
      let id = UntypedWindowId(self.window_id.next());
      let content = make_untyped(content);
      self.windows.insert(
         id,
         Window {
            view,
            content,
            data: Box::new(data),
         },
      );
      WindowId(id, PhantomData)
   }

   /// Closes an open window.
   pub fn close_window(&mut self, window: UntypedWindowId) {
      self.stack.retain(|f| f != &window);
      self.windows.remove(&window);
   }

   /// Processes windows inside the window manager.
   pub fn process(&mut self, ui: &mut Ui, input: &mut Input) {
      for stack_index in 0..self.stack.len() {
         let window_id = &self.stack[stack_index];
         let window = self.windows.get_mut(window_id).unwrap();
         let mut hit_test = Default::default();
         window.content.process(
            WindowContentArgs {
               ui,
               input,
               view: &window.view,
               hit_test: &mut hit_test,
            },
            &mut window.data,
         );
      }
   }
}

/// A unique window ID with the window's data type erased.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UntypedWindowId(usize);

/// A unique window ID, for a window storing data of type `D`.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct WindowId<D>(UntypedWindowId, PhantomData<D>);

/// Window content. Defines what's drawn on a window, and what data is shared between some other
/// part of the application, and the window content itself.
pub trait WindowContent {
   /// An arbitrarily-defined data value that's stored in the window.
   type Data;

   fn process(&mut self, args: WindowContentArgs, data: &mut Self::Data);
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

/// Window content, with the `Data` type erased.
trait UntypedWindowContent {
   fn process(&mut self, args: WindowContentArgs, data: &mut Box<dyn Any>);
}

/// Wraps a typed `WindowContent` in an `UntypedWindowContent`.
fn make_untyped<C, D>(content: C) -> Box<dyn UntypedWindowContent>
where
   C: WindowContent<Data = D> + 'static,
   D: Any,
{
   struct Wrapper<C> {
      inner: C,
   }

   impl<C, D> UntypedWindowContent for Wrapper<C>
   where
      C: WindowContent<Data = D> + 'static,
      D: Any,
   {
      fn process(&mut self, args: WindowContentArgs, data: &mut Box<dyn Any>) {
         let data: &mut D = data.downcast_mut().expect("downcasting window data failed");
         self.inner.process(args, data);
      }
   }

   let wrapper = Wrapper { inner: content };
   Box::new(wrapper)
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
