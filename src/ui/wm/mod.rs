//! A stacking window manager.

pub mod windows;

use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::assets::Assets;
use crate::common::VectorMath;
use crate::token::Token;

use super::view::View;
use super::{ButtonState, Input, Ui};

use crate::backend::winit::event::MouseButton;
use netcanv_renderer::paws::Layout;
pub use windows::WindowContentWrappers;

/// A window.
struct Window {
   /// The window's view.
   view: View,
   /// The window content.
   content: Box<dyn UntypedWindowContent>,
   /// The data shared between the window and its owner.
   data: Box<dyn Any>,

   /// Whether the window is pinned (won't be closed after you click away from it).
   pinned: bool,
   /// Whether the window was requested to be closed, by using the close button.
   close_requested: bool,
   /// Whether the window is currently being dragged.
   dragging: bool,
   /// Whether the window is the currently focused window.
   focused: bool,
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

   /// Steals the focus of the window manager, and focuses the window with the given ID.
   fn steal_focus(&mut self, id: UntypedWindowId) {
      for window_id in &self.stack {
         let window = self.windows.get_mut(window_id).unwrap();
         window.focused = &id == window_id;
      }
   }

   /// Returns whether any of the windows has focus.
   pub fn has_focus(&self) -> bool {
      self.stack.iter().map(|id| self.windows.get(id).unwrap()).any(|window| window.focused)
   }

   /// Opens a new window in the manager, and returns a handle for modifying it.
   pub fn open_window<C, D>(&mut self, view: View, content: C, data: D) -> WindowSettings<'_, D>
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
            focused: true,
         },
      );
      self.stack.push(id);
      self.steal_focus(id);
      WindowSettings {
         wm: self,
         id,
         _data: PhantomData,
      }
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
      let mut steal_focus = None;
      for stack_index in 0..self.stack.len() {
         let window_id = self.stack[stack_index];
         let window = self.windows.get_mut(&window_id).unwrap();
         let mut hit_test = Default::default();

         // Clone the view and floor its position such that the window renders perfectly on the
         // pixel grid. Winit may support subpixel precision with mouse coordinates, but that's
         // unwanted here.
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

         // Steal focus if the window was clicked.
         let mouse_clicked = input.global_mouse_button_just_pressed(MouseButton::Left)
            || input.global_mouse_button_just_pressed(MouseButton::Right);
         let mouse_clicked_inside_window = mouse_clicked && window.view.has_mouse(input);
         if mouse_clicked_inside_window {
            steal_focus = Some(window_id);
         }

         // Close the window if the user clicked away and it was unpinned, or if they
         // clicked the close button.
         let mouse_clicked_outside_window = mouse_clicked && !window.view.has_mouse(input);
         let close_button_clicked = hit_test == HitTest::CloseButton
            && input.action(MouseButton::Left) == (true, ButtonState::Released);
         if (mouse_clicked_outside_window && !window.pinned) || close_button_clicked {
            window.close_requested = true;
         }

         // If the pin button was clicked, toggle the window's pin state.
         if hit_test == HitTest::PinButton
            && input.action(MouseButton::Left) == (true, ButtonState::Released)
         {
            window.pinned = !window.pinned;
         }

         // Perform dragging if the mouse is over the draggable area.
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

      // The last window to write to steal_focus will get its focus state.
      // The order of operations here matters; we want the _last_ clicked window to steal focus as
      // that's the last one that got rendered to the screen.
      //
      // Do note that _no_ window could have been clicked.
      if let Some(window_id) = steal_focus {
         self.steal_focus(window_id);
      }
   }
}

/// A struct for changing a window's properties after creation.
pub struct WindowSettings<'wm, D> {
   wm: &'wm mut WindowManager,
   id: UntypedWindowId,
   _data: PhantomData<D>,
}

impl<'wm, D> WindowSettings<'wm, D> {
   /// Sets the pinned state of a window.
   pub fn set_pinned(self, pinned: bool) -> Self {
      self.wm.windows.get_mut(&self.id).unwrap().pinned = pinned;
      self
   }

   /// Finishes setting up a window.
   pub fn finish(self) -> WindowId<D> {
      WindowId(self.id, PhantomData)
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

   /// Processes the window content.
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
