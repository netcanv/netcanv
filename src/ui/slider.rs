//! A slider control.

use paws::{point, Color, Layout, Rect, Renderer};

use crate::common::quantize;
use crate::ui::*;

/// The step of a slider.
pub enum SliderStep {
   /// Smooth step - the slider can have any value.
   Smooth,
   /// Discrete step - the slider can only have values that are multiples of the given `f32`.
   Discrete(f32),
}

/// The state of a slider.
pub struct Slider {
   value: f32,
   min: f32,
   max: f32,
   step: SliderStep,
   sliding: bool,
}

/// Slider processing arguments.
#[derive(Clone, Copy)]
pub struct SliderArgs {
   pub width: f32,
   pub color: Color,
}

impl Slider {
   /// Creates a new slider state.
   pub fn new(value: f32, min: f32, max: f32, step: SliderStep) -> Self {
      Self {
         value: (value - min) / (max - min),
         min,
         max,
         step,
         sliding: false,
      }
   }

   /// Returns the number of steps on the slider, if it's discrete. Panics otherwise.
   fn step_count(&self) -> u32 {
      if let SliderStep::Discrete(step) = self.step {
         ((self.max - self.min) / step) as u32
      } else {
         panic!("attempt to use step_count on a non-discrete slider");
      }
   }

   /// Processes a slider.
   pub fn process(&mut self, ui: &mut Ui, input: &Input, SliderArgs { width, color }: SliderArgs) {
      ui.push((width, ui.height()), Layout::Freeform);

      match input.action(MouseButton::Left) {
         (true, ButtonState::Pressed) if ui.has_mouse(input) => self.sliding = true,
         (_, ButtonState::Released) => self.sliding = false,
         _ => (),
      }

      if self.sliding {
         self.value = ui.mouse_position(input).x / ui.width();
      }

      if ui.has_mouse(input) {
         if let (true, Some(scroll)) = input.action(MouseScroll) {
            let scroll_amount = match self.step {
               SliderStep::Discrete(increment) => increment / self.step_count() as f32 * 2.0,
               SliderStep::Smooth => 8.0 / width,
            };
            self.value += scroll.y * scroll_amount;
         }
      }

      self.value = self.value.clamp(0.0, 1.0);

      ui.draw(|ui| {
         let transparent = color.with_alpha(128);
         let mut x = self.value * ui.width();
         let y = ui.height() / 2.0;
         let width = ui.width();

         ui.render().fill(
            Rect::new(point(0.0, y - 1.0), vector(width, 2.0)),
            transparent,
            1.0,
         );

         if let SliderStep::Discrete(_) = self.step {
            let step_count = self.step_count();
            let norm_step = 1.0 / step_count as f32;
            let step_width = norm_step * ui.width();
            if step_width > 4.0 {
               for i in 0..=step_count {
                  let t = i as f32 * norm_step;
                  let px = t * ui.width();
                  ui.render().fill(Rect::new(point(px, y - 1.0), vector(2.0, 2.0)), color, 0.0);
               }
            }
            x = quantize(x, step_width);
         }

         ui.render().fill_circle(point(x, y), 5.0, color);
      });

      ui.pop();
   }

   /// Returns the slider's raw (normalized – unmapped) value (range [0.0; 1.0]).
   pub fn raw_value(&self) -> f32 {
      self.value
   }

   /// Returns the slider's value (mapped; range [min; max]).
   pub fn value(&self) -> f32 {
      let raw = (self.value * (self.max - self.min)) + self.min;
      match self.step {
         SliderStep::Smooth => raw,
         SliderStep::Discrete(step) => quantize(raw, step),
      }
   }

   /// Sets a new value for the slider (mapped; range [min; max]).
   pub fn set_value(&mut self, new_value: f32) {
      let raw = (new_value - self.min) / (self.max - self.min);
      let raw = raw.clamp(0.0, 1.0);
      self.value = raw;
   }

   /// Returns whether the slider is currently being slid around.
   pub fn is_sliding(&self) -> bool {
      self.sliding
   }
}
