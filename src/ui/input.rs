//! Simplified input handling facility.

use std::time::Instant;

use netcanv_renderer::paws::{vector, Point, Vector};
use winit::dpi::PhysicalPosition;
pub use winit::event::{ElementState, MouseButton, VirtualKeyCode};
use winit::event::{KeyboardInput, WindowEvent};

const MOUSE_BUTTON_COUNT: usize = 8;
const KEY_CODE_COUNT: usize = 256;

/// Input state.
pub struct Input {
   // mouse input
   mouse_position: Point,
   previous_mouse_position: Point,
   mouse_scroll: Vector,

   mouse_button_is_down: [bool; MOUSE_BUTTON_COUNT],
   mouse_button_just_pressed: [bool; MOUSE_BUTTON_COUNT],
   mouse_button_just_released: [bool; MOUSE_BUTTON_COUNT],
   active_mouse_area: u32,
   processed_mouse_area: u32,
   frame_mouse_area: u32,

   // keyboard input
   char_buffer: Vec<char>,

   key_just_typed: [bool; KEY_CODE_COUNT],
   key_is_down: [bool; KEY_CODE_COUNT],

   // time
   time_origin: Instant,
}

impl Input {
   /// Creates a new input state.
   pub fn new() -> Self {
      Self {
         mouse_position: Point::new(0.0, 0.0),
         previous_mouse_position: Point::new(0.0, 0.0),
         mouse_scroll: Vector::new(0.0, 0.0),

         mouse_button_is_down: [false; MOUSE_BUTTON_COUNT],
         mouse_button_just_pressed: [false; MOUSE_BUTTON_COUNT],
         mouse_button_just_released: [false; MOUSE_BUTTON_COUNT],
         active_mouse_area: 0,
         processed_mouse_area: 0,
         frame_mouse_area: 0,

         char_buffer: Vec::new(),
         key_just_typed: [false; KEY_CODE_COUNT],
         key_is_down: [false; KEY_CODE_COUNT],

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
      self.mouse_scroll
   }

   /// Returns whether mouse clicks are locked.
   fn mouse_buttons_locked(&self) -> bool {
      self.active_mouse_area != self.frame_mouse_area
   }

   /// Returns whether mouse events will be received.
   pub fn mouse_active(&self) -> bool {
      !self.mouse_buttons_locked()
   }

   /// Returns whether the given mouse button is being held down.
   pub fn mouse_button_is_down(&self, button: MouseButton) -> bool {
      if self.mouse_buttons_locked() {
         return false;
      }
      if let Some(i) = Self::mouse_button_index(button) {
         self.mouse_button_is_down[i]
      } else {
         false
      }
   }

   /// Returns whether the given mouse button has just been clicked.
   pub fn mouse_button_just_pressed(&self, button: MouseButton) -> bool {
      if self.mouse_buttons_locked() {
         return false;
      }
      if let Some(i) = Self::mouse_button_index(button) {
         self.mouse_button_just_pressed[i]
      } else {
         false
      }
   }

   /// Returns whether the given mouse button has just been released.
   pub fn mouse_button_just_released(&self, button: MouseButton) -> bool {
      if let Some(i) = Self::mouse_button_index(button) {
         self.mouse_button_just_released[i]
      } else {
         false
      }
   }

   /// Sets the _active mouse area_ for the current frame.
   ///
   /// Mouse events are only received if the mouse area at the end of the previous frame was the
   /// same as the mouse area that's currently active.
   pub fn set_mouse_area(&mut self, area: u32, active: bool) {
      self.active_mouse_area = area;
      if active {
         self.processed_mouse_area = area;
      }
   }

   /// Returns the characters that were typed during this frame.
   pub fn characters_typed(&self) -> &[char] {
      &self.char_buffer
   }

   /// Returns whether the provided key was just typed.
   pub fn key_just_typed(&self, key: VirtualKeyCode) -> bool {
      if let Some(i) = Self::key_index(key) {
         self.key_just_typed[i]
      } else {
         false
      }
   }

   /// Returns wheter the provided key is down
   pub fn key_is_down(&self, key: VirtualKeyCode) -> bool {
      if let Some(i) = Self::key_index(key) {
         self.key_is_down[i]
      } else {
         false
      }
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
            use winit::event::MouseScrollDelta::*;
            self.mouse_scroll = match *delta {
               LineDelta(x, y) => Vector::new(x, y),
               PixelDelta(PhysicalPosition { x, y }) => Vector::new(x as f32, y as f32),
            };
         }

         WindowEvent::ReceivedCharacter(c) => self.char_buffer.push(*c),

         WindowEvent::KeyboardInput {
            input:
               KeyboardInput {
                  state,
                  virtual_keycode: Some(key),
                  ..
               },
            ..
         } => self.process_keyboard_input(*key, *state),

         _ => (),
      }
   }

   /// Finishes an input frame. This resets pressed/released states, resets the previous mouse
   /// position, scroll delta, among other things, so this must be called at the end of each
   /// frame.
   pub fn finish_frame(&mut self) {
      for state in &mut self.mouse_button_just_pressed {
         *state = false;
      }
      for state in &mut self.mouse_button_just_released {
         *state = false;
      }
      self.previous_mouse_position = self.mouse_position;
      self.mouse_scroll = vector(0.0, 0.0);
      self.frame_mouse_area = self.processed_mouse_area;
      for state in &mut self.key_just_typed {
         *state = false;
      }
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
            }
            ElementState::Released => {
               self.mouse_button_is_down[i] = false;
               self.mouse_button_just_released[i] = true;
            }
         }
      }
   }

   /// Returns the numeric index of the key code, or `None` if the key code is not supported.
   fn key_index(key: VirtualKeyCode) -> Option<usize> {
      let i = key as usize;
      if i < KEY_CODE_COUNT {
         Some(i)
      } else {
         None
      }
   }

   /// Processes a keyboard input event.
   fn process_keyboard_input(&mut self, key: VirtualKeyCode, state: ElementState) {
      if let Some(i) = Self::key_index(key) {
         if state == ElementState::Pressed {
            self.key_just_typed[i] = true;
            self.key_is_down[i] = true;
         }

         if state == ElementState::Released {
            self.key_is_down[i] = false;
         }
      }
   }
}
