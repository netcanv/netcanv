//! A slider control.

use skulpin::app::MouseButton;
use skulpin::skia_safe::*;

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
    pub fn process(&mut self, ui: &mut Ui, canvas: &mut Canvas, input: &Input, SliderArgs { width, color }: SliderArgs) {
        ui.push_group((width, ui.height()), Layout::Freeform);

        if ui.has_mouse(input) && input.mouse_button_just_pressed(MouseButton::Left) {
            self.sliding = true;
        }
        if input.mouse_button_just_released(MouseButton::Left) {
            self.sliding = false;
        }

        if self.sliding {
            self.value = ui.mouse_position(input).x / ui.width();
            self.value = self.value.clamp(0.0, 1.0);
        }

        ui.draw_on_canvas(canvas, |canvas| {
            let transparent = Color4f::from(color.with_a(128));
            let mut paint = Paint::new(transparent, None);
            let mut x = self.value * ui.width();
            let y = ui.height() / 2.0;

            paint.set_anti_alias(true);
            paint.set_style(paint::Style::Stroke);
            paint.set_stroke_width(2.0);
            canvas.draw_line((0.0, y), (ui.width(), y), &paint);

            paint.set_color(paint.color().with_a(255));
            if let SliderStep::Discrete(_) = self.step {
                let step_count = self.step_count();
                let norm_step = 1.0 / step_count as f32;
                let step_width = norm_step * ui.width();
                if step_width > 4.0 {
                    for i in 0..=step_count {
                        let t = i as f32 * norm_step;
                        let px = t * ui.width();
                        canvas.draw_point((px, y), &paint);
                    }
                }
                x = quantize(x, step_width);
            }

            paint.set_style(paint::Style::Fill);
            canvas.draw_circle((x, y), 5.0, &paint);
        });

        ui.pop_group();
    }

    /// Returns the slider's value.
    pub fn value(&self) -> f32 {
        let raw = (self.value * (self.max - self.min)) + self.min;
        match self.step {
            SliderStep::Smooth => raw,
            SliderStep::Discrete(step) => quantize(raw, step),
        }
    }
}
