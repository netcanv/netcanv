//! A stacking window manager.

pub mod windows;

use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::assets::Assets;
use crate::common::VectorMath;
use crate::token::Token;

use super::view::View;
use super::{input, ButtonState, Input, Ui};

use netcanv_renderer::paws::Layout;
use netcanv_renderer_opengl::winit::event::MouseButton;
pub use windows::WindowContentWrappers;

/// A window.
struct Window {
   view: View,
   content: Box<dyn UntypedWindowContent>,
   data: Box<dyn Any>,
   pinned: bool,
   close_requested: bool,
   dragging: bool,
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

   /// Returns an immutable reference to the given window's data.
   pub fn window_data<D>(&self, id: &WindowId<D>) -> &D
   where
      D: Any,
   {
      self.windows.get(&id.0).unwrap().data.downcast_ref().unwrap()
   }

   /// Returns a mutable reference to the given window's data.
   pub fn window_data_mut<D>(&mut self, id: &WindowId<D>) -> &mut D
   where
      D: Any,
   {
      self.windows.get_mut(&id.0).unwrap().data.downcast_mut().unwrap()
   }

   /// Returns whether the window should close.
   pub fn should_close<D>(&self, id: &WindowId<D>) -> bool {
      self.windows.get(&id.0).unwrap().close_requested
   }

   /// Returns whether the window is pinned.
   pub fn pinned<D>(&self, id: &WindowId<D>) -> bool {
      self.windows.get(&id.0).unwrap().pinned
   }

   /// Returns a mutable reference to the window's view.
   pub fn view_mut<D>(&mut self, id: &WindowId<D>) -> &mut View {
      &mut self.windows.get_mut(&id.0).unwrap().view
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
            pinned: false,
            close_requested: false,
            dragging: false,
         },
      );
      self.stack.push(id);
      WindowId(id, PhantomData)
   }

   /// Closes an open window.
   pub fn close_window<D>(&mut self, window: WindowId<D>) -> D
   where
      D: Any,
   {
      self.stack.retain(|&f| f != window.0);
      let window = self.windows.remove(&window.0).unwrap();
      *window.data.downcast().unwrap()
   }

   /// Processes windows inside the window manager.
   pub fn process(&mut self, ui: &mut Ui, input: &mut Input, assets: &Assets) {
      for stack_index in 0..self.stack.len() {
         let window_id = &self.stack[stack_index];
         let window = self.windows.get_mut(window_id).unwrap();
         let mut hit_test = Default::default();

         let mut view = window.view.clone();
         view.position = view.position.floor();
         view.begin(ui, input, Layout::Freeform);
         window.content.process(
            &mut WindowContentArgs {
               ui,
               input,
               assets,
               view: &view,
               hit_test: &mut hit_test,
               pinned: window.pinned,
            },
            &mut window.data,
         );
         view.end(ui);

         let mouse_is_outside_of_window = (input
            .global_mouse_button_just_pressed(MouseButton::Left)
            || input.global_mouse_button_just_pressed(MouseButton::Right))
            && !window.pinned
            && !window.view.has_mouse(input);
         let close_button_clicked = hit_test == HitTest::CloseButton
            && input.action(MouseButton::Left) == (true, ButtonState::Released);
         if mouse_is_outside_of_window || close_button_clicked {
            window.close_requested = true;
         }

         if hit_test == HitTest::PinButton
            && input.action(MouseButton::Left) == (true, ButtonState::Released)
         {
            window.pinned = !window.pinned;
         }

         match input.action(MouseButton::Left) {
            (true, ButtonState::Pressed) if hit_test == HitTest::Draggable => {
               window.dragging = true;
               window.pinned = true;
            }
            (_, ButtonState::Released) => {
               window.dragging = false;
            }
            _ => (),
         }
         if window.dragging {
            window.view.position += input.mouse_position() - input.previous_mouse_position();
         }
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

   fn process(&mut self, args: &mut WindowContentArgs, data: &mut Self::Data);
}

/// Arguments passed to [`WindowContent::process`].
pub struct WindowContentArgs<'ui, 'input, 'process> {
   pub ui: &'ui mut Ui,
   pub input: &'input mut Input,
   pub assets: &'process Assets,
   pub view: &'process View,
   /// The hit test result.
   ///
   /// This can be set to determine the type of area the mouse cursor is under.
   /// See [`HitTest`]'s documentation for more information.
   pub hit_test: &'process mut HitTest,
   /// Whether the window is pinned.
   pub pinned: bool,
}

/// Window content, with the `Data` type erased.
trait UntypedWindowContent {
   fn process(&mut self, args: &mut WindowContentArgs, data: &mut Box<dyn Any>);
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
      fn process(&mut self, args: &mut WindowContentArgs, data: &mut Box<dyn Any>) {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTest {
   /// Window content. This is the default value of the hit test.
   Content,
   /// A draggable area, such as the title bar.
   Draggable,
   /// A button that can be clicked to close the window.
   CloseButton,
   /// A button that can be clicked to pin/unpin the window.
   PinButton,
}

impl Default for HitTest {
   fn default() -> Self {
      Self::Content
   }
}
