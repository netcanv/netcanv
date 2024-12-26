use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use web_time::Instant;

use netcanv_protocol::relay::PeerId;
use netcanv_renderer::paws::{point, vector, AlignH, AlignV, Color, Point, Rect, Renderer};
use netcanv_renderer::Font as FontTrait;
use netcanv_renderer::{BlendMode, RenderBackend};

use crate::assets::Assets;
use crate::backend::winit::event::MouseButton;
use crate::backend::winit::window::CursorIcon;
use crate::backend::{Backend, Image};
use crate::config::config;
use crate::keymap::KeyBinding;
use crate::paint::{
   self, deserialize_bincode, lerp_point, ColorMath, GlobalControls, RectMath, VectorMath,
};
use crate::ui::{
   view, Button, ButtonArgs, ButtonColors, ButtonState, ColorPicker, ColorPickerArgs, Tooltip,
   UiInput,
};
use crate::viewport::Viewport;

use super::{Net, Tool, ToolArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Shape {
   Rectangle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
   Idle,
   Drawing,
}

struct Icons {
   tool: Image,
   rectangle: Image,
}

pub struct ShapesTool {
   icons: Icons,

   state: State,
   shape: Shape,
   rect: Option<Rect>,
   peer_shapes: HashMap<PeerId, PeerShape>,
   mouse_position: Point,
   previous_mouse_position: Point,
}

impl ShapesTool {
   const MAX_SIZE: u32 = 1024;

   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icons: Icons {
            tool: Assets::load_svg(renderer, include_bytes!("../../../assets/icons/shape.svg")),
            rectangle: Assets::load_svg(
               renderer,
               include_bytes!("../../../assets/icons/rectangle.svg"),
            ),
         },

         state: State::Idle,
         shape: Shape::Rectangle,
         rect: None,
         peer_shapes: HashMap::new(),
         mouse_position: point(0.0, 0.0),
         previous_mouse_position: point(0.0, 0.0),
      }
   }

   /// Ensures that a peer's shape is properly initialized. Returns a mutable reference to
   /// said shape
   fn ensure_peer(&mut self, peer_id: PeerId) -> &mut PeerShape {
      self.peer_shapes.entry(peer_id).or_insert(PeerShape {
         shape: Shape::Rectangle,
         rect: None,
         previous_rect: None,
         last_rect_packet: Instant::now(),
         color: Color::BLACK,
         mouse_position: point(0.0, 0.0),
         previous_mouse_position: point(0.0, 0.0),
         last_cursor_packet: Instant::now(),
      })
   }

   fn draw_shape(renderer: &mut Backend, rect: Rect, color: Color) {
      renderer.fill(rect, color, 0.0);
   }

   fn draw_shape_on_paint_canvas(
      renderer: &mut Backend,
      paint_canvas: &mut paint::PaintCanvas,
      rect: Option<Rect>,
      color: Color,
   ) {
      if let Some(rect) = rect {
         paint_canvas.draw(renderer, rect, |renderer| {
            Self::draw_shape(renderer, rect, color);
         });
      }
   }

   fn color(global_controls: &GlobalControls) -> Color {
      global_controls.color_picker.color()
   }

   /// Returns a rounded, limited version of the shape
   fn normalized_rect(&self) -> Option<Rect> {
      self.rect.map(|rect| {
         let rect = Rect::new(
            rect.position,
            vector(
               rect.width().clamp(-(Self::MAX_SIZE as f32), Self::MAX_SIZE as f32),
               rect.height().clamp(-(Self::MAX_SIZE as f32), Self::MAX_SIZE as f32),
            ),
         );
         Rect::new(
            point(rect.x().floor(), rect.y().floor()),
            vector(rect.width().ceil(), rect.height().ceil()),
         )
      })
   }
}

impl Tool for ShapesTool {
   fn name(&self) -> &'static str {
      "shapes"
   }

   fn icon(&self) -> &Image {
      &self.icons.tool
   }

   fn key_shortcut(&self) -> KeyBinding {
      config().keymap.tools.shapes
   }

   fn process_paint_canvas_input(
      &mut self,
      ToolArgs {
         ui,
         input,
         global_controls,
         net,
         ..
      }: ToolArgs,
      paint_canvas: &mut crate::paint::PaintCanvas,
      viewport: &crate::viewport::Viewport,
   ) {
      // Calculate the mouse position.
      let mouse_position = ui.mouse_position(input);
      let mouse_position = viewport.to_viewport_space(mouse_position, ui.size());

      // Always show cursor as Crosshair
      input.set_cursor(CursorIcon::Crosshair);

      match input.action(MouseButton::Left) {
         (true, ButtonState::Pressed) => {
            // Start drawing
            self.state = State::Drawing;
            // Anchor the rectangle to the mouse position
            self.rect = Some(Rect::new(mouse_position, vector(0.0, 0.0)));
         }
         (_, ButtonState::Released) => {
            // Finish drawing
            Self::draw_shape_on_paint_canvas(
               ui.render(),
               paint_canvas,
               self.normalized_rect(),
               global_controls.color_picker.color(),
            );
            // Tell everyone we drew on the canvas
            catch!(net.send(self, PeerId::BROADCAST, Packet::Draw));
            self.state = State::Idle;
            self.rect = None;
         }
         _ => (),
      }

      // Change shape's size when moving the mouse
      if let Some(rect) = self.rect.as_mut() {
         if matches!(self.state, State::Drawing) {
            rect.size = mouse_position - rect.position;
         }
      }

      // Preserve current mouse position and previous mouse position so
      // we know when we should send cursor's position to peers
      self.previous_mouse_position = self.mouse_position;
      self.mouse_position = mouse_position;
   }

   /// Draws shapes overlay
   fn process_paint_canvas_overlays(
      &mut self,
      ToolArgs {
         ui,
         global_controls,
         ..
      }: ToolArgs,
      viewport: &Viewport,
   ) {
      if let Some(rect) = self.normalized_rect() {
         if !rect.is_smaller_than_a_pixel() {
            ui.draw(|ui| {
               let top_left = viewport.to_screen_space(rect.top_left(), ui.size()).floor();
               let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size()).floor();
               let rect = Rect::new(top_left, bottom_right - top_left);
               let renderer = ui.render();

               Self::draw_shape(renderer, rect, Self::color(global_controls));
            })
         }
      }
   }

   /// Processes peers' shape overlays
   fn process_paint_canvas_peer(
      &mut self,
      ToolArgs {
         ui, net, assets, ..
      }: ToolArgs,
      viewport: &Viewport,
      peer_id: PeerId,
   ) {
      if let Some(peer) = self.peer_shapes.get(&peer_id) {
         // Render peer's nickname
         let position = viewport.to_screen_space(peer.lerp_mouse_position(), ui.size());
         let renderer = ui.render();
         let nickname = net.peer_name(peer_id).unwrap();
         let text_color = if peer.color.brightness() < 0.5 || peer.color.a == 0 {
            Color::WHITE
         } else {
            Color::BLACK
         };
         let text_rect = Rect::new(
            position,
            vector(assets.sans.text_width(nickname), assets.sans.height()),
         );
         let padding = vector(4.0, 4.0);
         let text_rect = Rect::new(text_rect.position, text_rect.size + padding * 2.0);
         renderer.push();
         if peer.color.a == 0 {
            renderer.set_blend_mode(BlendMode::Invert);
            renderer.outline(text_rect, Color::WHITE, 2.0, 2.0);
         } else {
            renderer.fill(text_rect, peer.color, 2.0);
         }
         renderer.text(
            text_rect,
            &assets.sans,
            nickname,
            text_color,
            (AlignH::Center, AlignV::Middle),
         );
         renderer.pop();

         // Render peer's shape
         if let Some(rect) = peer.lerp_rect() {
            if !rect.is_smaller_than_a_pixel() {
               ui.draw(|ui| {
                  let top_left = viewport.to_screen_space(rect.top_left(), ui.size());
                  let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size());
                  let rect = Rect::new(top_left, bottom_right - top_left);

                  let renderer = ui.render();
                  Self::draw_shape(renderer, rect, peer.color);
               });
            }
         }
      }
   }

   /// Processes the color picker and shape buttons
   fn process_bottom_bar(
      &mut self,
      ToolArgs {
         ui,
         input,
         assets,
         wm,
         canvas_view,
         global_controls,
         ..
      }: ToolArgs,
   ) {
      // Draw the palette
      let mut picker_window = ColorPicker::picker_window_view();
      view::layout::align(
         &view::layout::padded(canvas_view, 16.0),
         &mut picker_window,
         (AlignH::Left, AlignV::Bottom),
      );
      global_controls.color_picker.process(
         ui,
         input,
         ColorPickerArgs {
            assets,
            wm,
            window_view: picker_window,
            show_eraser: false,
         },
      );
      ui.space(16.0);

      // Draw the shape buttons

      if Button::with_icon(
         ui,
         input,
         &ButtonArgs::new(
            ui,
            ButtonColors::toggle(
               self.shape == Shape::Rectangle,
               &assets.colors.toolbar_button,
               &assets.colors.selected_toolbar_button,
            ),
         )
         .tooltip(&assets.sans, Tooltip::top(&assets.tr.rectangle)),
         &self.icons.rectangle,
      )
      .clicked()
      {
         self.shape = Shape::Rectangle;
      }
   }

   fn network_send(&mut self, net: Net, global_controls: &GlobalControls) -> netcanv::Result<()> {
      // Update cursor's position when changed
      if self.mouse_position != self.previous_mouse_position {
         let Point { x, y } = self.mouse_position;
         let Color { r, g, b, a } = Self::color(global_controls);
         net.send(
            self,
            PeerId::BROADCAST,
            Packet::Cursor {
               position: (x, y),
               color: (r, g, b, a),
            },
         )?;
      }

      // If we are drawing a shape, tell it to other peers
      if let Some(rect) = self.normalized_rect() {
         let Color { r, g, b, a } = Self::color(global_controls);
         match self.shape {
            Shape::Rectangle => {
               net.send(
                  self,
                  PeerId::BROADCAST,
                  Packet::Update {
                     shape: self.shape,
                     color: (r, g, b, a),
                     position: (rect.x(), rect.y()),
                     size: (rect.width(), rect.height()),
                  },
               )?;
            }
         }
      }
      Ok(())
   }

   fn network_receive(
      &mut self,
      renderer: &mut Backend,
      _: Net,
      paint_canvas: &mut paint::PaintCanvas,
      peer_id: PeerId,
      payload: Vec<u8>,
   ) -> netcanv::Result<()> {
      let packet = deserialize_bincode(&payload)?;
      let peer = self.ensure_peer(peer_id);
      match packet {
         Packet::Cursor {
            position: (x, y),
            color: (r, g, b, a),
         } => {
            // Update peer's cursor
            peer.color = Color::new(r, g, b, a);
            peer.previous_mouse_position = peer.mouse_position;
            peer.mouse_position = point(x, y);
            peer.last_cursor_packet = Instant::now();
         }
         Packet::Update {
            shape,
            color: (r, g, b, a),
            position: (x, y),
            size: (width, height),
         } => {
            // Update peer's shape
            peer.shape = shape;
            peer.color = Color::new(r, g, b, a);
            peer.previous_rect = peer.rect;
            peer.rect = Some(Rect::new(
               point(x, y),
               vector(
                  width.min(Self::MAX_SIZE as f32),
                  height.min(Self::MAX_SIZE as f32),
               ),
            ));
            peer.last_rect_packet = Instant::now();
         }
         Packet::Draw => {
            // Draw peer's shape
            Self::draw_shape_on_paint_canvas(renderer, paint_canvas, peer.rect, peer.color);

            // Clear so the cursor doesn't jump around
            peer.previous_rect = None;
            peer.rect = None;
         }
      }
      Ok(())
   }

   // Sends to newly joined peer what are we doing right now
   fn network_peer_join(
      &mut self,
      _renderer: &mut Backend,
      net: Net,
      peer_id: PeerId,
      global_controls: &GlobalControls,
   ) -> netcanv::Result<()> {
      let Color { r, g, b, a } = Self::color(global_controls);
      match (self.state, self.shape) {
         (State::Idle, _) => {
            let Point { x, y } = self.mouse_position;
            net.send(
               self,
               peer_id,
               Packet::Cursor {
                  position: (x, y),
                  color: (r, g, b, a),
               },
            )?;
         }
         (State::Drawing, Shape::Rectangle) => {
            if let Some(rect) = self.normalized_rect() {
               net.send(
                  self,
                  peer_id,
                  Packet::Update {
                     shape: self.shape,
                     color: (r, g, b, a),
                     position: (rect.x(), rect.y()),
                     size: (rect.width(), rect.height()),
                  },
               )?;
            }
         }
      }

      Ok(())
   }

   fn network_peer_activate(&mut self, _net: Net, peer_id: PeerId) -> netcanv::Result<()> {
      self.ensure_peer(peer_id);
      Ok(())
   }
}

struct PeerShape {
   shape: Shape,
   color: Color,
   rect: Option<Rect>,
   previous_rect: Option<Rect>,
   last_rect_packet: Instant,
   mouse_position: Point,
   previous_mouse_position: Point,
   last_cursor_packet: Instant,
}

impl PeerShape {
   fn lerp_rect(&self) -> Option<Rect> {
      let elapsed = self.last_rect_packet.elapsed().as_millis() as f32;
      let t = (elapsed / paint::State::TIME_PER_UPDATE.as_millis() as f32).clamp(0.0, 1.0);
      self.rect.map(|mut rect| {
         let previous_rect = self.previous_rect.unwrap_or(rect);
         rect.position = lerp_point(previous_rect.position, rect.position, t);
         rect.size = lerp_point(previous_rect.size, rect.size, t);
         rect
      })
   }

   fn lerp_mouse_position(&self) -> Point {
      let elapsed_ms = self.last_cursor_packet.elapsed().as_millis() as f32;
      let t = (elapsed_ms / paint::State::TIME_PER_UPDATE.as_millis() as f32).min(1.0);
      lerp_point(self.previous_mouse_position, self.mouse_position, t)
   }
}

#[derive(Serialize, Deserialize)]
enum Packet {
   Cursor {
      position: (f32, f32),
      color: (u8, u8, u8, u8),
   },
   Update {
      shape: Shape,
      color: (u8, u8, u8, u8),
      position: (f32, f32),
      size: (f32, f32),
   },
   Draw,
}
