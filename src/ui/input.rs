use std::time::Instant;

use skulpin::skia_safe::*;

use winit::dpi::PhysicalPosition;
pub use winit::event::{ElementState, MouseButton, VirtualKeyCode};
use winit::event::{KeyboardInput, WindowEvent};

const MOUSE_BUTTON_COUNT: usize = 8;
const KEY_CODE_COUNT: usize = 256;

pub struct Input {
    // mouse input
    mouse_position: Point,
    previous_mouse_position: Point,

    mouse_button_is_down: [bool; MOUSE_BUTTON_COUNT],
    mouse_button_just_pressed: [bool; MOUSE_BUTTON_COUNT],
    mouse_button_just_released: [bool; MOUSE_BUTTON_COUNT],
    mouse_buttons_locked: bool,

    // keyboard input
    char_buffer: Vec<char>,
    key_just_typed: [bool; KEY_CODE_COUNT],

    // time
    time_origin: Instant,
}

impl Input {
    pub fn new() -> Self {
        Self {
            mouse_position: Point::new(0.0, 0.0),
            previous_mouse_position: Point::new(0.0, 0.0),
            mouse_button_is_down: [false; MOUSE_BUTTON_COUNT],
            mouse_button_just_pressed: [false; MOUSE_BUTTON_COUNT],
            mouse_button_just_released: [false; MOUSE_BUTTON_COUNT],
            mouse_buttons_locked: false,
            char_buffer: Vec::new(),
            key_just_typed: [false; KEY_CODE_COUNT],
            time_origin: Instant::now(),
        }
    }

    pub fn mouse_position(&self) -> Point {
        self.mouse_position
    }

    pub fn previous_mouse_position(&self) -> Point {
        self.previous_mouse_position
    }

    pub fn mouse_button_is_down(&self, button: MouseButton) -> bool {
        if self.mouse_buttons_locked {
            return false;
        }
        if let Some(i) = Self::mouse_button_index(button) {
            self.mouse_button_is_down[i]
        } else {
            false
        }
    }

    pub fn mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        if self.mouse_buttons_locked {
            return false;
        }
        if let Some(i) = Self::mouse_button_index(button) {
            self.mouse_button_just_pressed[i]
        } else {
            false
        }
    }

    pub fn mouse_button_just_released(&self, button: MouseButton) -> bool {
        if self.mouse_buttons_locked {
            return false;
        }
        if let Some(i) = Self::mouse_button_index(button) {
            self.mouse_button_just_released[i]
        } else {
            false
        }
    }

    pub fn lock_mouse_buttons(&mut self) {
        self.mouse_buttons_locked = true;
    }

    pub fn unlock_mouse_buttons(&mut self) {
        self.mouse_buttons_locked = false;
    }

    pub fn characters_typed(&self) -> &[char] {
        &self.char_buffer
    }

    pub fn key_just_typed(&self, key: VirtualKeyCode) -> bool {
        if let Some(i) = Self::key_index(key) {
            self.key_just_typed[i]
        } else {
            false
        }
    }

    pub fn time_in_seconds(&self) -> f32 {
        let now = self.time_origin.elapsed();
        now.as_millis() as f32 / 1_000.0
    }

    pub fn process_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let PhysicalPosition { x, y } = position;
                self.mouse_position = Point::new(*x as _, *y as _);
            }

            WindowEvent::MouseInput { button, state, .. } => {
                self.process_mouse_input(*button, *state)
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

    pub fn finish_frame(&mut self) {
        for state in &mut self.mouse_button_just_pressed {
            *state = false;
        }
        for state in &mut self.mouse_button_just_released {
            *state = false;
        }
        self.previous_mouse_position = self.mouse_position;
        for state in &mut self.key_just_typed {
            *state = false;
        }
        self.char_buffer.clear();
    }

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

    fn key_index(key: VirtualKeyCode) -> Option<usize> {
        let i = key as usize;
        if i < KEY_CODE_COUNT {
            Some(i)
        } else {
            None
        }
    }

    fn process_keyboard_input(&mut self, key: VirtualKeyCode, state: ElementState) {
        if let Some(i) = Self::key_index(key) {
            if state == ElementState::Pressed {
                self.key_just_typed[i] = true;
            }
        }
    }
}
