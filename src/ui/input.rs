//! Simplified input handling facility.

use instant::Instant;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::{BitAnd, BitOr};

use crate::backend::winit::dpi::PhysicalPosition;
pub use crate::backend::winit::event::{ElementState, MouseButton};
use crate::backend::winit::event::{KeyEvent, WindowEvent};
use crate::backend::winit::keyboard::{Key, ModifiersState};
use crate::backend::winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use crate::backend::winit::window::{CursorIcon, Window};
use netcanv_renderer::paws::{point, vector, Point, Vector};
use serde::de::Visitor;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

const MOUSE_BUTTON_COUNT: usize = 8;

/// Input state.
pub struct Input {
   // mouse input
   mouse_position: Point,
   previous_mouse_position: Point,
   mouse_scroll: Vector,

   mouse_button_is_down: [bool; MOUSE_BUTTON_COUNT],
   mouse_button_just_pressed: [bool; MOUSE_BUTTON_COUNT],
   mouse_button_just_released: [bool; MOUSE_BUTTON_COUNT],
   click_positions: [Point; MOUSE_BUTTON_COUNT],
   active_mouse_area: usize,
   processed_mouse_area: usize,
   frame_mouse_area: usize,

   previous_cursor: CursorIcon,
   cursor: CursorIcon,

   // keyboard input
   char_buffer: Vec<char>,

   key_just_typed: HashSet<Key>,
   key_is_down: HashSet<Key>,
   modifiers: ModifiersState,

   // time
   time_origin: Instant,
}

impl Input {
   /// Creates a new input state.
   pub fn new() -> Self {
      Self {
         mouse_position: point(0.0, 0.0),
         previous_mouse_position: point(0.0, 0.0),
         mouse_scroll: vector(0.0, 0.0),

         mouse_button_is_down: [false; MOUSE_BUTTON_COUNT],
         mouse_button_just_pressed: [false; MOUSE_BUTTON_COUNT],
         mouse_button_just_released: [false; MOUSE_BUTTON_COUNT],
         click_positions: [vector(0.0, 0.0); MOUSE_BUTTON_COUNT],
         active_mouse_area: 0,
         processed_mouse_area: 0,
         frame_mouse_area: 0,

         previous_cursor: CursorIcon::Default,
         cursor: CursorIcon::Default,

         char_buffer: Vec::new(),
         key_just_typed: HashSet::new(),
         key_is_down: HashSet::new(),
         modifiers: ModifiersState::default(),

         time_origin: Instant::now(),
      }
   }

   /// Returns the position of the mouse.
   pub fn mouse_position(&self) -> Point {
      self.mouse_position
   }

   /// Returns the position of the mouse, as it was on the previous frame.
   pub fn previous_mouse_position(&self) -> Point {
      self.previous_mouse_position
   }

   /// Returns the mouse's scroll delta.
   pub fn mouse_scroll(&self) -> Vector {
      if self.mouse_buttons_locked() {
         vector(0.0, 0.0)
      } else {
         self.mouse_scroll
      }
   }

   /// Returns whether mouse clicks are locked.
   fn mouse_buttons_locked(&self) -> bool {
      self.active_mouse_area != self.frame_mouse_area
   }

   /// Returns whether mouse events will be received.
   pub fn mouse_active(&self) -> bool {
      !self.mouse_buttons_locked()
   }

   /// Returns whether the given mouse button is being held down, globally (independent of
   /// the current mouse area).
   pub fn global_mouse_button_is_down(&self, button: MouseButton) -> bool {
      if let Some(i) = Self::mouse_button_index(button) {
         self.mouse_button_is_down[i]
      } else {
         false
      }
   }

   /// Returns whether the given mouse button is being held down.
   pub fn mouse_button_is_down(&self, button: MouseButton) -> bool {
      !self.mouse_buttons_locked() && self.global_mouse_button_is_down(button)
   }

   /// Returns whether the given mouse button has just been pressed, globally (independent of
   /// the current mouse area).
   pub fn global_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
      if let Some(i) = Self::mouse_button_index(button) {
         self.mouse_button_just_pressed[i]
      } else {
         false
      }
   }

   /// Returns whether the given mouse button has just been clicked.
   pub fn mouse_button_just_pressed(&self, button: MouseButton) -> bool {
      !self.mouse_buttons_locked() && self.global_mouse_button_just_pressed(button)
   }

   /// Returns whether the given mouse button has just been released.
   pub fn mouse_button_just_released(&self, button: MouseButton) -> bool {
      if let Some(i) = Self::mouse_button_index(button) {
         self.mouse_button_just_released[i]
      } else {
         false
      }
   }

   /// Returns the position where the last click with the given mouse button was initiated.
   pub fn click_position(&self, button: MouseButton) -> Point {
      if let Some(i) = Self::mouse_button_index(button) {
         self.click_positions[i]
      } else {
         point(0.0, 0.0)
      }
   }

   /// Sets the _active mouse area_ for the current frame.
   ///
   /// Mouse events are only received if the mouse area at the end of the previous frame was the
   /// same as the mouse area that's currently active.
   pub fn set_mouse_area(&mut self, area: usize, active: bool) {
      self.active_mouse_area = area;
      if active {
         self.processed_mouse_area = area;
      }
   }

   /// Sets the current mouse cursor.
   pub fn set_cursor(&mut self, cursor: CursorIcon) {
      self.cursor = cursor;
   }

   /// Returns the characters that were typed during this frame.
   pub fn characters_typed(&self) -> &[char] {
      &self.char_buffer
   }

   /// Returns whether the provided key was just typed.
   pub fn key_just_typed(&self, key: Key) -> bool {
      self.key_just_typed.contains(&key)
   }

   /// Returns whether the Ctrl key is being held down.
   pub fn ctrl_is_down(&self) -> bool {
      self.modifiers.control_key()
   }

   /// Returns whether the Shift key is being held down.
   pub fn shift_is_down(&self) -> bool {
      self.modifiers.shift_key()
   }

   /// Returns the time elapsed since this `Input` was created, in seconds.
   pub fn time_in_seconds(&self) -> f32 {
      let now = self.time_origin.elapsed();
      now.as_millis() as f32 / 1_000.0
   }

   /// Processes a `WindowEvent`.
   pub fn process_event(&mut self, event: &WindowEvent) {
      match event {
         WindowEvent::CursorMoved { position, .. } => {
            let PhysicalPosition { x, y } = position;
            self.mouse_position = Point::new(*x as _, *y as _);
         }

         WindowEvent::MouseInput { button, state, .. } => self.process_mouse_input(*button, *state),

         WindowEvent::MouseWheel { delta, .. } => {
            use crate::backend::winit::event::MouseScrollDelta::*;

            #[cfg(not(target_arch = "wasm32"))]
            {
               self.mouse_scroll = match *delta {
                  LineDelta(x, y) => Vector::new(x, y),
                  PixelDelta(PhysicalPosition { x, y }) => Vector::new(x as f32, y as f32),
               };
            }

            #[cfg(target_arch = "wasm32")]
            {
               self.mouse_scroll = match *delta {
                  LineDelta(x, y) => Vector::new(x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0)),
                  PixelDelta(PhysicalPosition { x, y }) => {
                     Vector::new(x.clamp(-1.0, 1.0) as f32, y.clamp(-1.0, 1.0) as f32)
                  }
               };
            }
         }

         WindowEvent::ModifiersChanged(new) => {
            self.modifiers = new.state();
         }

         WindowEvent::KeyboardInput { event, .. } => {
            let KeyEvent { state, text, .. } = event;

            if *state == ElementState::Pressed {
               if let Some(text) = text {
                  let chars: Vec<char> = text.chars().collect();
                  self.char_buffer.extend_from_slice(&chars);
               }
            }

            self.process_keyboard_input(event.key_without_modifiers(), *state)
         }

         _ => (),
      }
   }

   /// Finishes an input frame. This resets pressed/released states, resets the previous mouse
   /// position, scroll delta, among other things, so this must be called at the end of each
   /// frame.
   pub fn finish_frame(&mut self, window: &Window) {
      for state in &mut self.mouse_button_just_pressed {
         *state = false;
      }
      for state in &mut self.mouse_button_just_released {
         *state = false;
      }
      self.previous_mouse_position = self.mouse_position;
      self.mouse_scroll = vector(0.0, 0.0);
      self.frame_mouse_area = self.processed_mouse_area;
      if self.cursor != self.previous_cursor {
         self.previous_cursor = self.cursor;
         window.set_cursor_icon(self.cursor);
      }
      self.key_just_typed.clear();
      self.char_buffer.clear();
   }

   /// Returns the numeric index of the mouse given button, or `None` if the mouse button is not
   /// supported.
   fn mouse_button_index(button: MouseButton) -> Option<usize> {
      let i: usize = match button {
         MouseButton::Left => 0,
         MouseButton::Right => 1,
         MouseButton::Middle => 2,
         MouseButton::Other(x) => 3 + x as usize,
         MouseButton::Back | MouseButton::Forward => 99, // we don't care about those
      };

      if i < MOUSE_BUTTON_COUNT {
         Some(i)
      } else {
         None
      }
   }

   /// Processes a mouse input event.
   fn process_mouse_input(&mut self, button: MouseButton, state: ElementState) {
      if let Some(i) = Self::mouse_button_index(button) {
         match state {
            ElementState::Pressed => {
               self.mouse_button_is_down[i] = true;
               self.mouse_button_just_pressed[i] = true;
               self.click_positions[i] = self.mouse_position();
            }
            ElementState::Released => {
               self.mouse_button_is_down[i] = false;
               self.mouse_button_just_released[i] = true;
            }
         }
      }
   }

   /// Processes a keyboard input event.
   fn process_keyboard_input(&mut self, key: Key, state: ElementState) {
      if state == ElementState::Pressed {
         self.key_just_typed.insert(key.clone());
         self.key_is_down.insert(key.clone());
      }

      if state == ElementState::Released {
         self.key_is_down.remove(&key);
      }
   }
}

//
// Actions
//

/// A basic input action. This includes key presses, mouse clicks, etc., without modifier keys.
pub trait BasicAction {
   /// The result of the action. Usually a `bool`, but some actions, eg. mouse scrolling, can
   /// produce other things like scroll deltas.
   type Result;

   /// Checks whether the action is now being performed.
   fn check(&self, input: &Input) -> Self::Result;
}

/// The state of a mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
   /// A button is neither down, nor pressed, nor released.
   None,
   /// A button has just been pressed.
   Pressed,
   /// A button is being held down.
   Down,
   /// A button has just been released.
   Released,
}

impl BasicAction for MouseButton {
   type Result = ButtonState;

   fn check(&self, input: &Input) -> Self::Result {
      if input.mouse_button_just_pressed(*self) {
         ButtonState::Pressed
      } else if input.mouse_button_just_released(*self) {
         ButtonState::Released
      } else if input.mouse_button_is_down(*self) {
         ButtonState::Down
      } else {
         ButtonState::None
      }
   }
}

impl BasicAction for Key {
   type Result = bool;

   fn check(&self, input: &Input) -> Self::Result {
      input.key_just_typed(self.clone())
   }
}

/// Marker struct for the mouse scroll action.
pub struct MouseScroll;

impl BasicAction for MouseScroll {
   type Result = Option<Vector>;

   fn check(&self, input: &Input) -> Self::Result {
      if input.mouse_scroll().x != 0.0 || input.mouse_scroll().y != 0.0 {
         Some(input.mouse_scroll())
      } else {
         None
      }
   }
}

impl<A, const N: usize> BasicAction for [A; N]
where
   A: BasicAction,
{
   type Result = [A::Result; N];

   /// Checks all basic actions against the given input, and returns their results in an array.
   fn check(&self, input: &Input) -> Self::Result {
      let mut i = 0;
      [0; N].map(|_| {
         let r = self[i].check(input);
         i += 1;
         r
      })
   }
}

/// A full input action. This includes all basic actions, and actions with modifier keys.
///
/// Actions without modifier keys are treated as if they require no modifier keys to be held.
pub trait Action {
   /// The result of the action. See [`BasicAction::Result`].
   type Result;

   /// Checks whether the action is now being performed.
   fn check(&self, input: &Input) -> Self::Result;
}

impl Input {
   /// Checks the input state against an action.
   pub fn action<A>(&self, action: A) -> A::Result
   where
      A: Action,
   {
      action.check(self)
   }
}

/// A set of modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Modifier(u8);

impl Modifier {
   /// No modifier keys.
   pub const NONE: Self = Self(0);

   /// The Shift key.
   pub const SHIFT: Self = Self(0b1);
   /// The Ctrl key.
   pub const CTRL: Self = Self(0b10);

   const SHIFT_STR: &'static str = "Shift";
   const CTRL_STR: &'static str = "Ctrl";

   /// Creates modifiers from the given input.
   pub fn from_input(input: &Input) -> Self {
      let mut mods = Modifier(0);
      if input.shift_is_down() {
         mods = mods | Self::SHIFT;
      }
      if input.ctrl_is_down() {
         mods = mods | Self::CTRL;
      }
      mods
   }

   /// Returns whether the shift key is included in this set.
   pub fn shift(&self) -> bool {
      (*self & Self::SHIFT) == Self::SHIFT
   }

   /// Returns whether the control key is included in this set.
   pub fn ctrl(&self) -> bool {
      (*self & Self::CTRL) == Self::CTRL
   }

   /// Returns the cardinality of this set.
   pub fn card(&self) -> usize {
      self.shift() as usize + self.ctrl() as usize
   }
}

impl BitOr for Modifier {
   type Output = Self;

   /// Combines modifiers together.
   fn bitor(self, rhs: Self) -> Self::Output {
      Self(self.0 | rhs.0)
   }
}

impl BitAnd for Modifier {
   type Output = Self;

   /// Intersects modifiers together.
   fn bitand(self, rhs: Self) -> Self::Output {
      Self(self.0 & rhs.0)
   }
}

impl Serialize for Modifier {
   fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
   where
      S: serde::Serializer,
   {
      let mut seq = serializer.serialize_seq(Some(self.card()))?;
      if self.shift() {
         seq.serialize_element(Self::SHIFT_STR)?;
      }
      if self.ctrl() {
         seq.serialize_element(Self::CTRL_STR)?;
      }
      seq.end()
   }
}

impl<'de> Deserialize<'de> for Modifier {
   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
   where
      D: serde::Deserializer<'de>,
   {
      struct ModifierVisitor;

      impl<'de> Visitor<'de> for ModifierVisitor {
         type Value = Modifier;

         fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a set of modifier keys")
         }

         fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
         where
            A: serde::de::SeqAccess<'de>,
         {
            let mut modifier = Modifier::NONE;
            while let Some(element) = seq.next_element::<Cow<'_, str>>()? {
               match &*element {
                  Modifier::SHIFT_STR => modifier = modifier | Modifier::SHIFT,
                  Modifier::CTRL_STR => modifier = modifier | Modifier::CTRL,
                  _ => return Err(serde::de::Error::custom("invalid modifier")),
               }
            }
            Ok(modifier)
         }
      }

      deserializer.deserialize_seq(ModifierVisitor)
   }
}

impl<A> Action for (Modifier, A)
where
   A: BasicAction,
{
   /// The first tuple element specifies whether the modifier was satisfied. The second one
   /// is carried over from the other action in the pair.
   type Result = (bool, A::Result);

   fn check(&self, input: &Input) -> Self::Result {
      (Modifier::from_input(input) == self.0, self.1.check(input))
   }
}

impl<A> Action for &(Modifier, A)
where
   A: BasicAction,
{
   /// See the other implementation.
   type Result = (bool, A::Result);

   fn check(&self, input: &Input) -> Self::Result {
      (*self).check(input)
   }
}

impl<A> Action for A
where
   A: BasicAction,
{
   /// See the other implementation.
   type Result = (bool, A::Result);

   /// Checks the action against input state.
   /// Lone basic actions are the same as `(Modifier::NONE, self)`.
   fn check(&self, input: &Input) -> Self::Result {
      (
         Modifier::from_input(input) == Modifier::NONE,
         self.check(input),
      )
   }
}
