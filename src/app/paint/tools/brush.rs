//! The Brush tool. Allows for painting, as well as erasing pixels from the canvas.

use netcanv_renderer::paws::{point, vector, Color, LineCap, Point, Rect, Renderer};
use netcanv_renderer::{BlendMode, RenderBackend};
use winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::Image;
use crate::paint_canvas::PaintCanvas;
use crate::ui::{Input, Ui, UiInput};
use crate::viewport::Viewport;

use super::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrushState {
   Idle,
   Drawing,
   Erasing,
}

pub struct Brush {
   icon: Image,

   state: BrushState,
   thickness: f32,
   color: Color,

   position: Point,
}

impl Brush {
   pub fn new() -> Self {
      Self {
         icon: Assets::load_icon(include_bytes!("../../../assets/icons/brush.svg")),
         state: BrushState::Idle,
         thickness: 4.0,
         color: COLOR_PALETTE[0],
         position: point(0.0, 0.0),
      }
   }
}

impl Tool for Brush {
   fn name(&self) -> &str {
      "Brush"
   }

   fn icon(&self) -> &Image {
      &self.icon
   }

   /// Handles input and drawing to the paint canvas with the brush.
   fn process_paint_canvas_input(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      paint_canvas: &mut PaintCanvas,
      viewport: &Viewport,
   ) {
      // Read input.
      // We use the just_pressed/just_released pair of functions because _clicks_ specifically
      // may be blocked by other parts of the app, eg. when the mouse is over a panel.
      if input.mouse_button_just_pressed(MouseButton::Left) {
         self.state = BrushState::Drawing;
      } else if input.mouse_button_just_pressed(MouseButton::Right) {
         self.state = BrushState::Erasing;
      }
      if input.mouse_button_just_released(MouseButton::Left)
         || input.mouse_button_just_released(MouseButton::Right)
      {
         self.state = BrushState::Idle;
      }

      // Draw to the paint canvas.
      let a = ui.previous_mouse_position(input);
      let b = ui.mouse_position(input);
      let (a, b) = (
         viewport.to_viewport_space(a, ui.size()),
         viewport.to_viewport_space(b, ui.size()),
      );
      if self.state != BrushState::Idle {
         let thickness = vector(self.thickness, self.thickness);
         let coverage = Rect::new(a - thickness, b - a + thickness * 2.0).sort();
         paint_canvas.draw(ui, coverage, |renderer| {
            renderer.set_blend_mode(match self.state {
               BrushState::Idle => unreachable!(),
               BrushState::Drawing => BlendMode::Alpha,
               BrushState::Erasing => BlendMode::Clear,
            });
            renderer.line(a, b, self.color, LineCap::Round, self.thickness);
         });
      }
      self.position = b;
   }

   fn process_paint_canvas_overlays(&mut self, ui: &mut Ui, _input: &Input, viewport: &Viewport) {
      // Draw the guide circle.
      let position = viewport.to_screen_space(self.position, ui.size());
      let renderer = ui.render();
      renderer.push();
      // The circle is drawn with the Invert blend mode, such that it's visible on all
      // (well, most) backgrounds.
      // This doesn't work on 50% gray but this is the best we can do.
      renderer.set_blend_mode(BlendMode::Invert);
      let thickness = self.thickness * viewport.zoom();
      let thickness_offset = vector(thickness, thickness) / 2.0;
      renderer.outline(
         Rect::new(position - thickness_offset, thickness_offset * 2.0),
         Color::WHITE.with_alpha(240),
         thickness / 2.0,
         1.0,
      );
      renderer.pop();
   }
}

/// The palette of colors at the bottom of the screen.
const COLOR_PALETTE: &[Color] = &[
   Color::rgb(0x100820), // black
   Color::rgb(0xff003e), // red
   Color::rgb(0xff7b00), // orange
   Color::rgb(0xffff00), // yellow
   Color::rgb(0x2dd70e), // green
   Color::rgb(0x03cbfb), // aqua
   Color::rgb(0x0868eb), // blue
   Color::rgb(0xa315d7), // purple
   Color::rgb(0xffffff), // white
];
