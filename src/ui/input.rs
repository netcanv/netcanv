use skulpin::skia_safe::*;

use winit::dpi::PhysicalPosition;
pub use winit::event::{ElementState, MouseButton};
use winit::event::WindowEvent;

const MOUSE_BUTTON_COUNT: usize = 8;

pub struct Input {
    mouse_position: Point,

    mouse_button_is_down: [bool; MOUSE_BUTTON_COUNT],
    mouse_button_just_pressed: [bool; MOUSE_BUTTON_COUNT],
    mouse_button_just_released: [bool; MOUSE_BUTTON_COUNT],
}

impl Input {

    pub fn new() -> Self {
        Self {
            mouse_position: Point::new(0.0, 0.0),
            mouse_button_is_down: [false; MOUSE_BUTTON_COUNT],
            mouse_button_just_pressed: [false; MOUSE_BUTTON_COUNT],
            mouse_button_just_released: [false; MOUSE_BUTTON_COUNT],
        }
    }

    pub fn mouse_position(&self) -> Point {
        self.mouse_position
    }

    pub fn mouse_button_is_down(&self, button: MouseButton) -> bool {
        if let Some(i) = Self::mouse_button_index(button) {
            self.mouse_button_is_down[i]
        } else {
            false
        }
    }

    pub fn mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        if let Some(i) = Self::mouse_button_index(button) {
            self.mouse_button_just_pressed[i]
        } else {
            false
        }
    }

    pub fn mouse_button_just_released(&self, button: MouseButton) -> bool {
        if let Some(i) = Self::mouse_button_index(button) {
            self.mouse_button_just_released[i]
        } else {
            false
        }
    }

    pub fn process_event(&mut self, event: &WindowEvent) {
        match event {

            WindowEvent::CursorMoved { position, .. } => {
                let PhysicalPosition { x, y } = position;
                self.mouse_position = Point::new(*x as _, *y as _);
            },

            WindowEvent::MouseInput { button, state, .. } => {
                self.process_mouse_input(*button, *state);
            },

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
        let maybe_i = Self::mouse_button_index(button);
        if let Some(i) = maybe_i {
            match state {
                ElementState::Pressed => {
                    self.mouse_button_is_down[i] = true;
                    self.mouse_button_just_pressed[i] = true;
                },
                ElementState::Released => {
                    self.mouse_button_is_down[i] = false;
                    self.mouse_button_just_released[i] = true;
                },
            }
        }
    }

}
