//! The Brush tool. Allows for painting, as well as erasing pixels from the canvas.

use netcanv_renderer::paws::{point, vector, Color, Layout, LineCap, Point, Rect, Renderer};
use netcanv_renderer::{BlendMode, RenderBackend};
use winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::Image;
use crate::paint_canvas::PaintCanvas;
use crate::ui::{Slider, SliderArgs, SliderStep, UiElements, UiInput};
use crate::viewport::Viewport;

use super::{Tool, ToolArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrushState {
   Idle,
   Drawing,
   Erasing,
}

pub struct Brush {
   icon: Image,

   state: BrushState,
   thickness_slider: Slider,
   color: Color,

   position: Point,
}

impl Brush {
   const MAX_THICKNESS: f32 = 64.0;

   /// Creates an instance of the brush tool.
   pub fn new() -> Self {
      Self {
         icon: Assets::load_icon(include_bytes!("../../../assets/icons/brush.svg")),
         state: BrushState::Idle,
         thickness_slider: Slider::new(4.0, 1.0, Self::MAX_THICKNESS, SliderStep::Discrete(1.0)),
         color: COLOR_PALETTE[0],
         position: point(0.0, 0.0),
      }
   }

   /// Returns the brush thickness.
   fn thickness(&self) -> f32 {
      self.thickness_slider.value()
   }

   /// Returns the coverage rectangle for the provided point.
   fn point_coverage(&self, p: Point) -> Rect {
      let half_thickness = self.thickness() / 2.0;
      Rect::new(
         point(p.x - half_thickness, p.y - half_thickness),
         vector(self.thickness(), self.thickness()),
      )
   }

   /// Returns the coverage rectangle for the two points.
   fn coverage(&self, a: Point, b: Point) -> Rect {
      let a_coverage = self.point_coverage(a);
      let b_coverage = self.point_coverage(b);
      let left = a_coverage.left().min(b_coverage.left());
      let top = a_coverage.top().min(b_coverage.top());
      let right = a_coverage.right().max(b_coverage.right());
      let bottom = a_coverage.bottom().max(b_coverage.bottom());
      Rect::new(point(left, top), vector(right - left, bottom - top))
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
      ToolArgs { ui, input, .. }: ToolArgs,
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
         let coverage = self.coverage(a, b);
         paint_canvas.draw(ui, coverage, |renderer| {
            renderer.set_blend_mode(match self.state {
               BrushState::Idle => unreachable!(),
               BrushState::Drawing => BlendMode::Alpha,
               BrushState::Erasing => BlendMode::Clear,
            });
            renderer.line(a, b, self.color, LineCap::Round, self.thickness());
         });
      }
      self.position = b;
   }

   /// Draws the guide circle of the brush.
   fn process_paint_canvas_overlays(
      &mut self,
      ToolArgs { ui, input, .. }: ToolArgs,
      viewport: &Viewport,
   ) {
      if input.mouse_active() {
         // Draw the guide circle.
         let position = viewport.to_screen_space(self.position, ui.size());
         let renderer = ui.render();
         renderer.push();
         // The circle is drawn with the Invert blend mode, such that it's visible on all
         // (well, most) backgrounds.
         // This doesn't work on 50% gray but this is the best we can do.
         renderer.set_blend_mode(BlendMode::Invert);
         let thickness = self.thickness() * viewport.zoom();
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

   /// Processes the color picker and brush size slider on the bottom bar.
   fn process_bottom_bar(&mut self, ToolArgs { ui, input, assets }: ToolArgs) {
      // Draw the palette.

      for &color in COLOR_PALETTE {
         ui.push((16.0, ui.height()), Layout::Freeform);
         let y_offset = ui.height()
            * if self.color == color {
               0.5
            } else if ui.has_mouse(&input) {
               0.7
            } else {
               0.8
            };
         let y_offset = y_offset.round();
         if ui.has_mouse(&input) && input.mouse_button_just_pressed(MouseButton::Left) {
            self.color = color;
         }
         ui.draw(|ui| {
            let rect = Rect::new(point(0.0, y_offset), ui.size());
            ui.render().fill(rect, color, 4.0);
         });
         ui.pop();
      }
      ui.space(16.0);

      // Draw the thickness: its slider and value display.

      ui.label(&assets.sans, "Thickness", assets.colors.text, None);
      ui.space(16.0);

      ui.push((192.0, ui.height()), Layout::Freeform);
      self.thickness_slider.process(
         ui,
         input,
         SliderArgs {
            width: ui.width(),
            color: assets.colors.slider,
         },
      );
      // Draw the size indicator above the slider.
      if self.thickness_slider.is_sliding() {
         ui.draw(|ui| {
            let size =
               (self.thickness() + (self.thickness() / Self::MAX_THICKNESS * 8.0 + 8.0)).max(32.0);
            let x = self.thickness_slider.raw_value() * ui.width() - size / 2.0;
            let renderer = ui.render();
            let rect = Rect::new(point(x, -size - 8.0), vector(size, size));
            renderer.fill(rect, assets.colors.panel, 8.0);
            renderer.outline_circle(
               rect.center(),
               self.thickness() / 2.0,
               assets.colors.text,
               1.0,
            );
         });
      }
      ui.pop();
      ui.space(8.0);

      ui.label(
         &assets.sans_bold,
         &self.thickness().to_string(),
         assets.colors.text,
         Some(ui.height()),
      );
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
