//! The Brush tool. Allows for painting, as well as erasing pixels from the canvas.

use std::net::SocketAddr;

use netcanv_renderer::paws::{point, vector, Color, Layout, LineCap, Point, Rect, Renderer};
use netcanv_renderer::{BlendMode, RenderBackend};
use serde::{Deserialize, Serialize};
use winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::{Backend, Image};
use crate::paint_canvas::PaintCanvas;
use crate::ui::{Slider, SliderArgs, SliderStep, UiElements, UiInput};
use crate::viewport::Viewport;

use super::{Net, Tool, ToolArgs};

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

   mouse_position: Point,
   stroke_points: Vec<Stroke>,
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
         mouse_position: point(0.0, 0.0),
         stroke_points: Vec::new(),
      }
   }

   /// Returns the brush thickness.
   fn thickness(&self) -> f32 {
      self.thickness_slider.value()
   }

   /// Returns the coverage rectangle for the provided point.
   fn point_coverage(p: Point, thickness: f32) -> Rect {
      let half_thickness = thickness / 2.0;
      Rect::new(
         point(p.x - half_thickness, p.y - half_thickness),
         vector(thickness, thickness),
      )
   }

   /// Returns the coverage rectangle for the two points.
   fn coverage(a: Point, b: Point, thickness: f32) -> Rect {
      let a_coverage = Self::point_coverage(a, thickness);
      let b_coverage = Self::point_coverage(b, thickness);
      let left = a_coverage.left().min(b_coverage.left());
      let top = a_coverage.top().min(b_coverage.top());
      let right = a_coverage.right().max(b_coverage.right());
      let bottom = a_coverage.bottom().max(b_coverage.bottom());
      Rect::new(point(left, top), vector(right - left, bottom - top))
   }

   fn stroke(
      &self,
      renderer: &mut Backend,
      paint_canvas: &mut PaintCanvas,
      a: Point,
      b: Point,
      color: Option<Color>,
      thickness: f32,
   ) {
      let coverage = Self::coverage(a, b, thickness);
      paint_canvas.draw(renderer, coverage, |renderer| {
         let color = match color {
            Some(color) => color,
            None => {
               renderer.set_blend_mode(BlendMode::Clear);
               Color::BLACK
            }
         };
         renderer.line(a, b, color, LineCap::Round, thickness);
      });
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
         self.stroke(
            ui,
            paint_canvas,
            a,
            b,
            match self.state {
               BrushState::Drawing => Some(self.color),
               BrushState::Erasing => None,
               _ => unreachable!(),
            },
            self.thickness(),
         );
         self.stroke_points.push(Stroke {
            color: match self.state {
               BrushState::Drawing => {
                  Some((self.color.r, self.color.g, self.color.b, self.color.a))
               }
               BrushState::Erasing => None,
               _ => unreachable!(),
            },
            thickness: self.thickness() as u8,
            a: (a.x, a.y),
            b: (b.x, b.y),
         });
      }
      self.mouse_position = b;
   }

   /// Draws the guide circle of the brush.
   fn process_paint_canvas_overlays(
      &mut self,
      ToolArgs { ui, input, .. }: ToolArgs,
      viewport: &Viewport,
   ) {
      if input.mouse_active() {
         // Draw the guide circle.
         let position = viewport.to_screen_space(self.mouse_position, ui.size());
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
   fn process_bottom_bar(
      &mut self,
      ToolArgs {
         ui, input, assets, ..
      }: ToolArgs,
   ) {
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

   fn network_send(&mut self, net: Net) -> anyhow::Result<()> {
      if !self.stroke_points.is_empty() {
         let packet = Packet::Stroke(self.stroke_points.drain(..).collect());
         net.send(self, packet)?;
      }
      Ok(())
   }

   fn network_receive(
      &mut self,
      renderer: &mut Backend,
      _net: Net,
      paint_canvas: &mut PaintCanvas,
      _sender: SocketAddr,
      payload: Vec<u8>,
   ) -> anyhow::Result<()> {
      let packet: Packet = bincode::deserialize(&payload)?;
      match packet {
         Packet::Cursor((x, y)) => todo!(),
         Packet::Stroke(points) => {
            for Stroke {
               color,
               thickness,
               a,
               b,
            } in points
            {
               // Verify that the packet is correct.
               let thickness = thickness as f32;
               // With thickness being a float, we allow for a little bit of leeway because
               // computers are dumb.
               anyhow::ensure!(thickness <= Self::MAX_THICKNESS + 0.1);
               // Draw the stroke.
               let a = {
                  let (ax, ay) = a;
                  point(ax, ay)
               };
               let b = {
                  let (bx, by) = b;
                  point(bx, by)
               };
               let color = color.map(|(r, g, b, a)| Color::new(r, g, b, a));
               self.stroke(renderer, paint_canvas, a, b, color, thickness);
            }
         }
      }
      Ok(())
   }
}

#[derive(Serialize, Deserialize)]
struct Stroke {
   color: Option<(u8, u8, u8, u8)>,
   thickness: u8,
   a: (f32, f32),
   b: (f32, f32),
}

/// A brush packet.
#[derive(Serialize, Deserialize)]
enum Packet {
   Cursor((f32, f32)),
   Stroke(Vec<Stroke>),
}

struct PeerBrush {
   mouse_position: Point,
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
