//! The Brush tool. Allows for painting, as well as erasing pixels from the canvas.

use std::collections::HashMap;
use std::time::Instant;

use crate::backend::winit::event::MouseButton;
use crate::config::config;
use netcanv_protocol::relay::PeerId;
use netcanv_renderer::paws::{
   point, vector, AlignH, AlignV, Color, Layout, LineCap, Point, Rect, Renderer,
};
use netcanv_renderer::{BlendMode, Font, RenderBackend};
use netcanv_renderer_opengl::winit::event::VirtualKeyCode;
use serde::{Deserialize, Serialize};

use crate::app::paint;
use crate::assets::Assets;
use crate::backend::{Backend, Image};
use crate::common::{lerp_point, ColorMath};
use crate::paint_canvas::PaintCanvas;
use crate::ui::{
   view, ButtonState, ColorPicker, ColorPickerArgs, Modifier, MouseScroll, Slider, SliderArgs,
   SliderStep, UiElements, UiInput,
};
use crate::viewport::Viewport;

use super::{Net, Tool, ToolArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrushState {
   Idle,
   Drawing,
   Erasing,
}

pub struct BrushTool {
   icon: Image,

   state: BrushState,
   thickness_slider: Slider,
   color_picker: ColorPicker,

   mouse_position: Point,
   previous_mouse_position: Point,
   stroke_points: Vec<Stroke>,

   peers: HashMap<PeerId, PeerBrush>,
}

impl BrushTool {
   const MAX_THICKNESS: f32 = 64.0;

   /// Creates an instance of the brush tool.
   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icon: Assets::load_icon(renderer, include_bytes!("../../../assets/icons/brush.svg")),
         state: BrushState::Idle,
         thickness_slider: Slider::new(4.0, 1.0, Self::MAX_THICKNESS, SliderStep::Discrete(1.0)),
         color_picker: ColorPicker::new(),
         mouse_position: point(0.0, 0.0),
         previous_mouse_position: point(0.0, 0.0),
         stroke_points: Vec::new(),
         peers: HashMap::new(),
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
      color: Color,
      thickness: f32,
   ) {
      let coverage = Self::coverage(a, b, thickness);
      renderer.push();
      renderer.set_blend_mode(BlendMode::Replace);
      paint_canvas.draw(renderer, coverage, |renderer| {
         renderer.line(a, b, color, LineCap::Round, thickness);
      });
      renderer.pop();
   }

   fn ensure_peer(&mut self, peer_id: PeerId) -> &mut PeerBrush {
      if !self.peers.contains_key(&peer_id) {
         self.peers.insert(
            peer_id,
            PeerBrush {
               mouse_position: point(0.0, 0.0),
               previous_mouse_position: point(0.0, 0.0),
               last_cursor_packet: Instant::now(),
               thickness: 4.0,
               color: Color::BLACK,
            },
         );
      }
      self.peers.get_mut(&peer_id).unwrap()
   }

   /// Returns the color currently selected in the color picker.
   pub fn color(&self) -> Color {
      self.color_picker.color()
   }
}

impl Tool for BrushTool {
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

      match input.action([MouseButton::Left, MouseButton::Right]) {
         (true, [ButtonState::Pressed, _]) => self.state = BrushState::Drawing,
         (true, [_, ButtonState::Pressed]) => self.state = BrushState::Erasing,
         (_, [ButtonState::Released, _] | [_, ButtonState::Released]) => {
            self.state = BrushState::Idle
         }
         _ => (),
      }

      // Shortcuts: Ctrl+Scroll, Ctrl+- and Ctrl+= can be used to alter the brush size.

      let mut thickness_change = 0.0;

      if let (true, Some(scroll)) = input.action((Modifier::CTRL, MouseScroll)) {
         thickness_change += scroll.y * 2.0;
      }

      if input.action(config().keymap.brush.decrease_thickness) == (true, true) {
         thickness_change -= 2.0;
      }
      if input.action(config().keymap.brush.increase_thickness) == (true, true) {
         thickness_change += 2.0;
      }

      self.thickness_slider.set_value(self.thickness() + thickness_change);

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
               BrushState::Drawing => self.color(),
               BrushState::Erasing => Color::TRANSPARENT,
               _ => unreachable!(),
            },
            self.thickness(),
         );
         self.stroke_points.push(Stroke {
            color: match self.state {
               BrushState::Drawing => (
                  self.color().r,
                  self.color().g,
                  self.color().b,
                  self.color().a,
               ),
               BrushState::Erasing => (0, 0, 0, 0),
               _ => unreachable!(),
            },
            thickness: self.thickness() as u8,
            a: (a.x, a.y),
            b: (b.x, b.y),
         });
      }
      self.previous_mouse_position = self.mouse_position;
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
         renderer.outline_circle(
            position,
            self.thickness() / 2.0 * viewport.zoom(),
            Color::WHITE.with_alpha(240),
            1.0,
         );
         renderer.pop();
      }
   }

   /// Processes the guide circle of a peer.
   fn process_paint_canvas_peer(
      &mut self,
      ToolArgs {
         ui, net, assets, ..
      }: ToolArgs,
      viewport: &Viewport,
      peer_id: PeerId,
   ) {
      if let Some(peer) = self.peers.get(&peer_id) {
         let position = viewport.to_screen_space(peer.lerp_mouse_position(), ui.size());
         let radius = peer.thickness / 2.0 * viewport.zoom();
         let renderer = ui.render();
         // Render their guide circle.
         renderer.push();
         renderer.set_blend_mode(BlendMode::Invert);
         renderer.outline_circle(position, radius, Color::WHITE.with_alpha(240), 1.0);
         renderer.pop();
         // Render their nickname.
         let nickname = net.peer_name(peer_id).unwrap();
         let text_color = if peer.color.brightness() > 0.5 {
            Color::BLACK
         } else {
            Color::WHITE
         };
         let thickness = vector(radius, radius);
         let text_rect = Rect::new(
            position + thickness,
            vector(assets.sans.text_width(nickname), assets.sans.height()),
         );
         let padding = vector(4.0, 4.0);
         let text_rect = Rect::new(text_rect.position, text_rect.size + padding * 2.0);
         renderer.fill(text_rect, peer.color, 2.0);
         renderer.text(
            text_rect,
            &assets.sans,
            nickname,
            text_color,
            (AlignH::Center, AlignV::Middle),
         );
      }
   }

   /// Processes the color picker and brush size slider on the bottom bar.
   fn process_bottom_bar(
      &mut self,
      ToolArgs {
         ui,
         input,
         assets,
         wm,
         canvas_view,
         ..
      }: ToolArgs,
   ) {
      // Draw the palette.
      let mut picker_window = ColorPicker::picker_window_view();
      view::layout::align(
         &view::layout::padded(canvas_view, 16.0),
         &mut picker_window,
         (AlignH::Left, AlignV::Bottom),
      );
      self.color_picker.process(
         ui,
         input,
         ColorPickerArgs {
            assets,
            wm,
            window_view: picker_window,
         },
      );
      ui.space(16.0);

      // Draw the thickness: its slider and value display.
      ui.horizontal_label(&assets.sans, "Thickness", assets.colors.text, None);
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

      // Draw the thickness text.
      ui.horizontal_label(
         &assets.sans_bold,
         &self.thickness().to_string(),
         assets.colors.text,
         Some((ui.height(), AlignH::Center)),
      );
   }

   fn network_send(&mut self, net: Net) -> anyhow::Result<()> {
      if !self.stroke_points.is_empty() {
         let packet = Packet::Stroke(self.stroke_points.drain(..).collect());
         net.send(self, PeerId::BROADCAST, packet)?;
      }
      if self.mouse_position != self.previous_mouse_position {
         let Point { x, y } = self.mouse_position;
         let Color { r, g, b, a } = self.color();
         net.send(
            self,
            PeerId::BROADCAST,
            Packet::Cursor {
               position: (x, y),
               thickness: self.thickness() as u8,
               color: (r, g, b, a),
            },
         )?;
      }
      Ok(())
   }

   fn network_receive(
      &mut self,
      renderer: &mut Backend,
      _net: Net,
      paint_canvas: &mut PaintCanvas,
      sender: PeerId,
      payload: Vec<u8>,
   ) -> anyhow::Result<()> {
      let packet: Packet = bincode::deserialize(&payload)?;
      match packet {
         Packet::Cursor {
            position: (x, y),
            thickness,
            color: (r, g, b, a),
         } => {
            let peer = self.ensure_peer(sender);
            peer.previous_mouse_position = peer.mouse_position;
            peer.mouse_position = point(x, y);
            peer.last_cursor_packet = Instant::now();
            peer.thickness = thickness as f32;
            peer.color = Color::new(r, g, b, a);
         }
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
               let color = {
                  let (r, g, b, a) = color;
                  Color::new(r, g, b, a)
               };
               self.stroke(renderer, paint_canvas, a, b, color, thickness);
            }
         }
      }
      Ok(())
   }

   fn network_peer_activate(&mut self, _net: Net, peer_id: PeerId) -> anyhow::Result<()> {
      self.ensure_peer(peer_id);
      Ok(())
   }
}

#[derive(Serialize, Deserialize)]
struct Stroke {
   color: (u8, u8, u8, u8),
   thickness: u8,
   a: (f32, f32),
   b: (f32, f32),
}

/// A brush packet.
#[derive(Serialize, Deserialize)]
enum Packet {
   Cursor {
      position: (f32, f32),
      thickness: u8,
      color: (u8, u8, u8, u8),
   },
   Stroke(Vec<Stroke>),
}

struct PeerBrush {
   mouse_position: Point,
   previous_mouse_position: Point,
   last_cursor_packet: Instant,
   thickness: f32,
   color: Color,
}

impl PeerBrush {
   fn lerp_mouse_position(&self) -> Point {
      let elapsed_ms = self.last_cursor_packet.elapsed().as_millis() as f32;
      let t = (elapsed_ms / paint::State::TIME_PER_UPDATE.as_millis() as f32).min(1.0);
      lerp_point(self.previous_mouse_position, self.mouse_position, t)
   }
}
