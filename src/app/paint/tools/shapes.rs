use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use web_time::Instant;

use netcanv_protocol::relay::PeerId;
use netcanv_renderer::paws::{point, vector, AlignH, AlignV, Color, Point, Rect, Renderer};
use netcanv_renderer::RenderBackend;
use netcanv_renderer::{BlendMode, Font as FontTrait};

use crate::assets::Assets;
use crate::backend::winit::event::MouseButton;
use crate::backend::winit::window::CursorIcon;
use crate::backend::{Backend, Image};
use crate::config::config;
use crate::keymap::KeyBinding;
use crate::paint::{
   self, deserialize_bincode, format_vector, label_width, lerp_point, ColorMath, GlobalControls,
   PaintCanvas, RectMath, VectorMath,
};
use crate::ui::{
   view, Button, ButtonArgs, ButtonColors, ButtonState, ColorPicker, ColorPickerArgs, Tooltip,
   UiElements, UiInput,
};
use crate::viewport::Viewport;

use super::{KeyShortcutAction, Net, Tool, ToolArgs};

struct Icons {
   tool: Image,
   cursor: Image,
   position: Image,
   rectangle: Image,
   ellipse: Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Shape {
   Rectangle,
   Ellipse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handle {
   TopLeft,
   Top,
   TopRight,
   Right,
   BottomRight,
   Bottom,
   BottomLeft,
   Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
   None,
   Drawing,
   DraggingHandle(Handle),
   DraggingWhole,
}

impl Action {
   fn or(self, rhs: Self) -> Self {
      if self == Self::None {
         rhs
      } else {
         self
      }
   }
}

fn normalize_rect(rect: &Option<Rect>) -> Option<Rect> {
   rect.map(|rect| {
      let rect = Rect::new(
         rect.position,
         vector(
            rect.width().clamp(-(ShapesTool::MAX_SIZE as f32), ShapesTool::MAX_SIZE as f32),
            rect.height().clamp(-(ShapesTool::MAX_SIZE as f32), ShapesTool::MAX_SIZE as f32),
         ),
      );
      Rect::new(
         point(rect.x().floor(), rect.y().floor()),
         vector(rect.width().ceil(), rect.height().ceil()),
      )
   })
}

fn draw_shape(renderer: &mut Backend, shape: Shape, rect: Rect, color: Color) {
   match shape {
      Shape::Rectangle => renderer.fill(rect, color, 0.0),
      Shape::Ellipse => {
         let radius_x = (rect.x() - rect.center_x()).abs();
         let radius_y = (rect.y() - rect.center_y()).abs();
         renderer.fill_with_radiuses(rect.sort(), color, (radius_x, radius_y));
      }
   }
}

#[derive(Debug)]
enum State {
   Shape { shape: Shape, rect: Option<Rect> },
   Selection(Selection),
}

impl State {
   /// Returns a shape or a selection with a rounded, limited version of the rectangle.
   fn with_normalized_rect(&self) -> Self {
      match self {
         State::Shape { shape, rect } => {
            let rect = normalize_rect(rect);

            Self::Shape {
               shape: *shape,
               rect,
            }
         }
         State::Selection(selection) => State::Selection(selection.with_normalized_rect()),
      }
   }

   /// Returns a rounded, limited version of the rectangle.
   fn normalized_rect(&self) -> Option<Rect> {
      match self.with_normalized_rect() {
         Self::Shape { rect, .. } => rect,
         Self::Selection(Selection { rect, .. }) => rect,
      }
   }
}

pub struct ShapesTool {
   icons: Icons,
   mouse_position: Point,
   previous_mouse_position: Point,
   potential_action: Action,
   action: Action,
   shape: Shape,
   state: State,
   peer_shapes: HashMap<PeerId, PeerShape>,
}

impl ShapesTool {
   const HANDLE_COLOR: Color = Color::rgb(0x0397fb);
   const HANDLE_RADIUS: f32 = 4.0;
   const MAX_SIZE: u32 = 1024;

   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icons: Icons {
            tool: Assets::load_svg(renderer, include_bytes!("../../../assets/icons/shape.svg")),
            cursor: Assets::load_svg(
               renderer,
               include_bytes!("../../../assets/icons/position.svg"),
            ),
            position: Assets::load_svg(
               renderer,
               include_bytes!("../../../assets/icons/selection-position.svg"),
            ),
            rectangle: Assets::load_svg(
               renderer,
               include_bytes!("../../../assets/icons/rectangle.svg"),
            ),
            ellipse: Assets::load_svg(
               renderer,
               include_bytes!("../../../assets/icons/ellipse.svg"),
            ),
         },
         mouse_position: point(0.0, 0.0),
         previous_mouse_position: point(0.0, 0.0),
         potential_action: Action::None,
         action: Action::None,
         shape: Shape::Rectangle,
         state: State::Shape {
            shape: Shape::Rectangle,
            rect: None,
         },
         peer_shapes: HashMap::new(),
      }
   }

   /// Returns whether the mouse cursor is hovered over a handle.
   fn hovered_handle(rect: Rect, point: Point, handle_radius: f32) -> Option<Handle> {
      if point.is_in_circle(rect.top_left(), handle_radius) {
         Some(Handle::TopLeft)
      } else if point.is_in_circle(rect.top_center(), handle_radius) {
         Some(Handle::Top)
      } else if point.is_in_circle(rect.top_right(), handle_radius) {
         Some(Handle::TopRight)
      } else if point.is_in_circle(rect.right_center(), handle_radius) {
         Some(Handle::Right)
      } else if point.is_in_circle(rect.bottom_right(), handle_radius) {
         Some(Handle::BottomRight)
      } else if point.is_in_circle(rect.bottom_center(), handle_radius) {
         Some(Handle::Bottom)
      } else if point.is_in_circle(rect.bottom_left(), handle_radius) {
         Some(Handle::BottomLeft)
      } else if point.is_in_circle(rect.left_center(), handle_radius) {
         Some(Handle::Left)
      } else {
         None
      }
   }

   /// Draws a resize handle.
   fn draw_handle(&self, renderer: &mut Backend, position: Point, handle: Handle) {
      let radius = if self.potential_action == Action::DraggingHandle(handle) {
         Self::HANDLE_RADIUS * 2.0
      } else {
         Self::HANDLE_RADIUS
      };
      renderer.fill_circle(position, radius + 2.0, Color::WHITE);
      renderer.fill_circle(position, radius, Self::HANDLE_COLOR);
   }

   fn color(global_controls: &GlobalControls) -> Color {
      global_controls.color_picker.color()
   }

   fn selection_rect(&self) -> Option<Rect> {
      if let State::Selection(selection) = &self.state {
         selection.rect
      } else {
         None
      }
   }

   fn selection_normalized_rect(&self) -> Option<Rect> {
      if let State::Selection(selection) = &self.state.with_normalized_rect() {
         selection.rect
      } else {
         None
      }
   }

   /// Sends an `Update` packet containing the shape's position, size and color.
   /// This is sometimes needed before important actions, where the shape may not have been
   /// synchronized yet due to the lower network tick rate.
   fn send_update_packet(
      &self,
      net: &Net,
      global_controls: &GlobalControls,
   ) -> netcanv::Result<()> {
      if let Some(rect) = self.state.normalized_rect() {
         let Color { r, g, b, a } = match self.state {
            State::Shape { .. } => Self::color(global_controls),
            State::Selection(Selection { color, .. }) => color,
         };
         net.send(
            self,
            PeerId::BROADCAST,
            Packet::Update {
               position: (rect.x(), rect.y()),
               size: (rect.width(), rect.height()),
               color: (r, g, b, a),
            },
         )?;
      }
      Ok(())
   }

   /// Ensures that a peer's shape is properly initialized. Returns a mutable reference to
   /// said shape
   fn ensure_peer(&mut self, peer_id: PeerId) -> &mut PeerShape {
      self.peer_shapes.entry(peer_id).or_insert(PeerShape {
         state: PeerState::Shape {
            shape: Shape::Rectangle,
            rect: None,
            color: Color::BLACK,
         },
         previous_normalized_rect: None,
         last_rect_packet: Instant::now(),
         mouse_position: point(0.0, 0.0),
         previous_mouse_position: point(0.0, 0.0),
         cursor_color: Color::BLACK,
         last_cursor_packet: Instant::now(),
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
      config().keymap.tools.selection
   }

   /// When the tool is deactivated, the selection should be deselected.
   fn deactivate(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      if let State::Selection(selection) = &mut self.state {
         selection.deselect(renderer, paint_canvas);
      }
   }

   /// Processes key shortcuts when the selection is active.
   fn active_key_shortcuts(
      &mut self,
      ToolArgs { input, net, .. }: ToolArgs,
      _paint_canvas: &mut PaintCanvas,
      _viewport: &Viewport,
   ) -> KeyShortcutAction {
      if input.action(config().keymap.edit.delete) == (true, true) {
         if let State::Selection(selection) = &mut self.state {
            selection.cancel();
            catch!(
               net.send(self, PeerId::BROADCAST, Packet::Cancel),
               return KeyShortcutAction::None
            );
         }
         return KeyShortcutAction::Success;
      }

      KeyShortcutAction::None
   }

   /// Processes mouse input.
   fn process_paint_canvas_input(
      &mut self,
      ToolArgs {
         ui,
         input,
         net,
         global_controls,
         ..
      }: ToolArgs,
      paint_canvas: &mut PaintCanvas,
      viewport: &Viewport,
   ) {
      // Calculate the mouse position.
      let mouse_position = ui.mouse_position(input);
      let mouse_position = viewport.to_viewport_space(mouse_position, ui.size());
      let previous_mouse_position = ui.previous_mouse_position(input);
      let previous_mouse_position = viewport.to_viewport_space(previous_mouse_position, ui.size());
      // Preserve current mouse position and previous mouse position so
      // we know when we should send cursor's position to peers
      self.mouse_position = mouse_position;
      self.previous_mouse_position = previous_mouse_position;

      let handle_radius = Self::HANDLE_RADIUS * 3.0 / viewport.zoom();
      self.potential_action = Action::Drawing;
      // Only let the user resize or drag the selection if they aren't doing anything at the moment.
      if matches!(self.action, Action::None | Action::DraggingWhole) {
         if let Some(rect) = self.selection_rect() {
            // Check the handles.
            if let Some(handle) = Self::hovered_handle(rect, mouse_position, handle_radius) {
               self.potential_action = Action::DraggingHandle(handle);
            } else {
               // Check the inside.
               let rect = Rect::new(
                  rect.position - vector(4.0, 4.0) / viewport.zoom(),
                  rect.size + vector(8.0, 8.0) / viewport.zoom(),
               )
               .sort();
               if mouse_position.is_in_rect(rect) {
                  self.potential_action = Action::DraggingWhole;
               }
            }
         }
      }

      input.set_cursor(match self.action.or(self.potential_action) {
         Action::None => CursorIcon::Crosshair,
         Action::Drawing => CursorIcon::Crosshair,
         Action::DraggingHandle(_) => {
            // We process the hovered handles for a second time, because the first time around the
            // rectangle was not sorted.
            if let Some(rect) = self.selection_normalized_rect() {
               if let Some(handle) = Self::hovered_handle(rect, mouse_position, handle_radius) {
                  match handle {
                     Handle::Left | Handle::Right => CursorIcon::ColResize,
                     Handle::Top | Handle::Bottom => CursorIcon::RowResize,
                     Handle::TopLeft | Handle::BottomRight => CursorIcon::NwseResize,
                     Handle::BottomLeft | Handle::TopRight => CursorIcon::NeswResize,
                  }
               } else {
                  CursorIcon::Default
               }
            } else {
               CursorIcon::Default
            }
         }
         Action::DraggingWhole => CursorIcon::AllScroll,
      });

      // Check if the left mouse button was pressed, and if so, start drawing.
      match input.action(MouseButton::Left) {
         (true, ButtonState::Pressed) => {
            if self.potential_action == Action::Drawing {
               if let State::Selection(selection) = &mut self.state {
                  // Before we start drawing, draw the shape from the selection onto the canvas.
                  selection.deselect(ui, paint_canvas);
                  catch!(self.send_update_packet(&net, global_controls));
                  catch!(net.send(self, PeerId::BROADCAST, Packet::Deselect));
               }
               self.state = State::Shape {
                  shape: self.shape,
                  rect: Some(Rect::new(mouse_position, vector(0.0, 0.0))),
               };
               tracing::trace!("changed state into shape");
               catch!(self.send_update_packet(&net, global_controls));
            }
            self.action = self.potential_action;
         }
         (_, ButtonState::Released) => {
            match &mut self.state {
               State::Selection(selection) => {
                  // After the button is released and the selection's size is close to 0, deselect.
                  if let Some(rect) = selection.rect {
                     if rect.is_smaller_than_a_pixel() {
                        selection.cancel();
                        catch!(net.send(self, PeerId::BROADCAST, Packet::Cancel));
                     }
                  }
               }
               State::Shape { rect, .. } => {
                  // Clear the shape's rect so it doesn't turn into a selection
                  if let Some(r) = rect {
                     if r.is_smaller_than_a_pixel() {
                        *rect = None;
                     }
                  }
               }
            }
            if self.action == Action::Drawing {
               if let State::Shape {
                  shape,
                  rect: Some(rect),
               } = self.state.with_normalized_rect()
               {
                  // User is done drawing, so we turn the shape into a selection.
                  let selection = Selection {
                     shape,
                     rect: Some(rect),
                     color: Self::color(&global_controls),
                     deselected_at: None,
                  };
                  self.state = State::Selection(selection);
                  tracing::trace!("changed state into selection");
                  catch!(net.send(self, PeerId::BROADCAST, Packet::Selection));
               }
            }
            self.action = Action::None;
         }
         _ => (),
      }

      // Resize the shape when user is moving the mouse.
      if let State::Shape {
         rect: Some(rect), ..
      } = &mut self.state
      {
         if matches!(self.action, Action::Drawing) {
            rect.size = mouse_position - rect.position;
         }
      }

      // Perform all the actions.
      if let State::Selection(selection) = &mut self.state {
         if let Some(rect) = selection.rect.as_mut() {
            match self.action {
               Action::None | Action::Drawing => (),
               Action::DraggingHandle(handle) => {
                  let new_rect = match handle {
                     Handle::TopLeft => rect.with_top_left(mouse_position),
                     Handle::Top => rect.with_top(mouse_position.y),
                     Handle::TopRight => rect.with_top_right(mouse_position),
                     Handle::Right => rect.with_right(mouse_position.x),
                     Handle::BottomRight => rect.with_bottom_right(mouse_position),
                     Handle::Bottom => rect.with_bottom(mouse_position.y),
                     Handle::BottomLeft => rect.with_bottom_left(mouse_position),
                     Handle::Left => rect.with_left(mouse_position.x),
                  };
                  // Prevent the selection, that is at its max size, from being moved
                  // by dragging a handle.
                  //
                  // This works by limiting minimum X to the X of the top right corner
                  // minus selection's max size, and limiting maximum X to the X of the
                  // top right corner plus selection's max size. The same for Y, but with
                  // bottom left instead of top right.
                  //
                  // If top right corner is at X = 0, then minimum X would be -1024,
                  // and maximum X would be 1024, and same for Y if Y = 0. Selection's position
                  // is calculated (new_rect) and then new_rect.x and new_rect.y are restricted
                  // to the interval with clamp. So, e.g. new_rect.x = -1100, will become -1024,
                  // and selection won't move.
                  *rect = Rect::new(
                     point(
                        new_rect.x().clamp(
                           rect.top_right().x - Selection::MAX_SIZE as f32,
                           rect.top_right().x + Selection::MAX_SIZE as f32,
                        ),
                        new_rect.y().clamp(
                           rect.bottom_left().y - Selection::MAX_SIZE as f32,
                           rect.bottom_left().y + Selection::MAX_SIZE as f32,
                        ),
                     ),
                     new_rect.size,
                  );
                  selection.rect = selection.normalized_rect();
               }
               Action::DraggingWhole => {
                  let delta_position = mouse_position - previous_mouse_position;
                  rect.position += delta_position;
               }
            }
         }
      }
   }

   /// Processes the overlays.
   fn process_paint_canvas_overlays(
      &mut self,
      ToolArgs {
         ui,
         global_controls,
         ..
      }: ToolArgs,
      viewport: &Viewport,
   ) {
      // Shape overlay
      if let State::Shape {
         shape,
         rect: Some(rect),
      } = self.state.with_normalized_rect()
      {
         if !rect.is_smaller_than_a_pixel() {
            ui.draw(|ui| {
               let top_left = viewport.to_screen_space(rect.top_left(), ui.size()).floor();
               let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size()).floor();
               let rect = Rect::new(top_left, bottom_right - top_left);
               let renderer = ui.render();

               draw_shape(renderer, shape, rect, Self::color(global_controls));
            })
         }
      }

      // Selection overlay
      if let State::Selection(Selection {
         rect: Some(rect),
         shape,
         color,
         ..
      }) = self.state.with_normalized_rect()
      {
         if !rect.is_smaller_than_a_pixel() {
            ui.draw(|ui| {
               let top_left = viewport.to_screen_space(rect.top_left(), ui.size()).floor();
               let top = viewport.to_screen_space(rect.top_center(), ui.size()).floor();
               let top_right = viewport.to_screen_space(rect.top_right(), ui.size()).floor();
               let right = viewport.to_screen_space(rect.right_center(), ui.size()).floor();
               let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size()).floor();
               let bottom = viewport.to_screen_space(rect.bottom_center(), ui.size()).floor();
               let bottom_left = viewport.to_screen_space(rect.bottom_left(), ui.size()).floor();
               let left = viewport.to_screen_space(rect.left_center(), ui.size()).floor();
               let rect = Rect::new(top_left, bottom_right - top_left);
               let renderer = ui.render();
               draw_shape(renderer, shape, rect, color);
               renderer.outline(
                  rect,
                  Self::HANDLE_COLOR,
                  0.0,
                  if self.potential_action == Action::DraggingWhole {
                     4.0
                  } else {
                     2.0
                  },
               );
               self.draw_handle(renderer, top_left, Handle::TopLeft);
               self.draw_handle(renderer, top, Handle::Top);
               self.draw_handle(renderer, top_right, Handle::TopRight);
               self.draw_handle(renderer, right, Handle::Right);
               self.draw_handle(renderer, bottom_right, Handle::BottomRight);
               self.draw_handle(renderer, bottom, Handle::Bottom);
               self.draw_handle(renderer, bottom_left, Handle::BottomLeft);
               self.draw_handle(renderer, left, Handle::Left);
            });
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
         net,
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
         catch!(net.send(self, PeerId::BROADCAST, Packet::Shape(Shape::Rectangle)));
      }

      if Button::with_icon(
         ui,
         input,
         &ButtonArgs::new(
            ui,
            ButtonColors::toggle(
               self.shape == Shape::Ellipse,
               &assets.colors.toolbar_button,
               &assets.colors.selected_toolbar_button,
            ),
         )
         .tooltip(&assets.sans, Tooltip::top(&assets.tr.ellipse)),
         &self.icons.ellipse,
      )
      .clicked()
      {
         self.shape = Shape::Ellipse;
         catch!(net.send(self, PeerId::BROADCAST, Packet::Shape(Shape::Ellipse)));
      }

      if let State::Selection(selection) = &mut self.state {
         let icon_size = vector(ui.height(), ui.height());

         // Show the mouse position.
         let mouse_position = format_vector(self.mouse_position);
         ui.icon(&self.icons.cursor, assets.colors.text, Some(icon_size));
         ui.horizontal_label(
            &assets.sans,
            &mouse_position,
            assets.colors.text,
            Some((label_width(&assets.sans, &mouse_position), AlignH::Center)),
         );

         if let Some(rect) = selection.normalized_rect() {
            let rect = rect.sort();
            // Show the selection anchor.
            let anchor = format_vector(rect.position);
            ui.icon(&self.icons.position, assets.colors.text, Some(icon_size));
            ui.horizontal_label(
               &assets.sans,
               &anchor,
               assets.colors.text,
               Some((label_width(&assets.sans, &anchor), AlignH::Center)),
            );
            let size = format!("{:.0} \u{00d7} {:.0}", rect.width(), rect.height());
            ui.icon(&self.icons.rectangle, assets.colors.text, Some(icon_size));
            ui.horizontal_label(
               &assets.sans,
               &size,
               assets.colors.text,
               Some((label_width(&assets.sans, &size), AlignH::Center)),
            );
         }
      }
   }

   /// Processes peers' overlays.
   fn process_paint_canvas_peer(
      &mut self,
      ToolArgs {
         ui, net, assets, ..
      }: ToolArgs,
      viewport: &Viewport,
      peer_id: PeerId,
   ) {
      if let Some(peer) = self.peer_shapes.get(&peer_id) {
         if let Some(rect) = peer.lerp_normalized_rect() {
            if !rect.is_smaller_than_a_pixel() {
               match &peer.state {
                  PeerState::Shape { shape, color, .. } => ui.draw(|ui| {
                     let top_left = viewport.to_screen_space(rect.top_left(), ui.size());
                     let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size());
                     let rect = Rect::new(top_left, bottom_right - top_left);

                     let renderer = ui.render();
                     draw_shape(renderer, *shape, rect, *color);
                  }),
                  PeerState::Selection(selection) => {
                     ui.draw(|ui| {
                        let top_left = viewport.to_screen_space(rect.top_left(), ui.size());
                        let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size());
                        let rect = Rect::new(top_left, bottom_right - top_left);

                        let nickname = net.peer_name(peer_id).unwrap();
                        let text_width = assets.sans.text_width(nickname);
                        let padding = vector(4.0, 4.0);
                        let text_rect = Rect::new(
                           top_left,
                           vector(text_width, assets.sans.height()) + padding * 2.0,
                        );

                        let renderer = ui.render();
                        draw_shape(renderer, selection.shape, rect, selection.color);
                        renderer.outline(rect, Self::HANDLE_COLOR, 0.0, 2.0);
                        if rect.width() > text_rect.width() && rect.height() > text_rect.height() {
                           renderer.fill(text_rect, Self::HANDLE_COLOR, 2.0);
                           renderer.text(
                              text_rect,
                              &assets.sans,
                              nickname,
                              Color::WHITE,
                              (AlignH::Center, AlignV::Middle),
                           );
                        }
                     });
                  }
               }
            }
         }

         // Render peer's nickname
         let position = viewport.to_screen_space(peer.lerp_mouse_position(), ui.size());
         let renderer = ui.render();
         let nickname = net.peer_name(peer_id).unwrap();
         let text_color = if peer.cursor_color.brightness() < 0.5 || peer.cursor_color.a == 0 {
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
         if peer.cursor_color.a == 0 {
            renderer.set_blend_mode(BlendMode::Invert);
            renderer.outline(text_rect, Color::WHITE, 2.0, 2.0);
         } else {
            renderer.fill(text_rect, peer.cursor_color, 2.0);
         }
         renderer.text(
            text_rect,
            &assets.sans,
            nickname,
            text_color,
            (AlignH::Center, AlignV::Middle),
         );
         renderer.pop();
      }
   }

   /// Sends out packets containing the rectangle and color.
   fn network_send(&mut self, net: Net, global_controls: &GlobalControls) -> netcanv::Result<()> {
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

      self.send_update_packet(&net, global_controls)?;
      Ok(())
   }

   /// Interprets an incoming packet.
   fn network_receive(
      &mut self,
      renderer: &mut Backend,
      _: Net,
      paint_canvas: &mut PaintCanvas,
      peer_id: PeerId,
      payload: Vec<u8>,
   ) -> netcanv::Result<()> {
      let packet = deserialize_bincode(&payload)?;
      let peer = self.ensure_peer(peer_id);
      match packet {
         Packet::Update {
            position: (x, y),
            size: (width, height),
            color: (r, g, b, a),
         } => {
            peer.previous_normalized_rect = peer.state.normalized_rect();
            peer.state.set_rect(Rect::new(
               point(x, y),
               vector(
                  width.min(ShapesTool::MAX_SIZE as f32),
                  height.min(ShapesTool::MAX_SIZE as f32),
               ),
            ));
            peer.state.set_color(Color::new(r, g, b, a));
            peer.last_rect_packet = Instant::now();
         }
         Packet::Selection => {
            if let PeerState::Shape {
               shape, rect, color, ..
            } = &peer.state
            {
               peer.state = PeerState::Selection(Selection {
                  shape: *shape,
                  rect: *rect,
                  color: *color,
                  deselected_at: None,
               });
            }
         }
         Packet::Cancel => {
            if let PeerState::Selection(selection) = &mut peer.state {
               selection.cancel();
            }
         }
         Packet::Deselect => {
            if let PeerState::Selection(selection) = &mut peer.state {
               selection.deselect(renderer, paint_canvas);
            }
         }
         Packet::Shape(shape) => peer.state.set_shape(shape),
         Packet::Cursor {
            position: (x, y),
            color: (r, g, b, a),
         } => {
            // Update peer's cursor
            peer.cursor_color = Color::new(r, g, b, a);
            peer.previous_mouse_position = peer.mouse_position;
            peer.mouse_position = point(x, y);
            peer.last_cursor_packet = Instant::now();
         }
      }
      Ok(())
   }

   /// Sends a capture packet to the peer that joined.
   fn network_peer_join(
      &mut self,
      _: &mut Backend,
      net: Net,
      peer_id: PeerId,
      global_controls: &GlobalControls,
   ) -> netcanv::Result<()> {
      match self.state.with_normalized_rect() {
         State::Shape { shape, rect } => {
            let Color { r, g, b, a } = Self::color(global_controls);
            net.send(self, peer_id, Packet::Shape(shape))?;
            if let Some(rect) = rect {
               net.send(
                  self,
                  peer_id,
                  Packet::Update {
                     position: (rect.x(), rect.y()),
                     size: (rect.width(), rect.height()),
                     color: (r, g, b, a),
                  },
               )?;
            }
         }
         State::Selection(Selection {
            rect,
            shape,
            color: Color { r, g, b, a },
            ..
         }) => {
            net.send(self, peer_id, Packet::Shape(shape))?;
            if let Some(rect) = rect {
               net.send(
                  self,
                  peer_id,
                  Packet::Update {
                     position: (rect.x(), rect.y()),
                     size: (rect.width(), rect.height()),
                     color: (r, g, b, a),
                  },
               )?;
               net.send(self, peer_id, Packet::Selection)?;
            }
         }
      }

      if let Some(rect) = self.state.normalized_rect() {
         let Color { r, g, b, a } = match self.state {
            State::Shape { .. } => Self::color(global_controls),
            State::Selection(Selection { color, .. }) => color,
         };
         net.send(
            self,
            peer_id,
            Packet::Update {
               position: (rect.x(), rect.y()),
               size: (rect.width(), rect.height()),
               color: (r, g, b, a),
            },
         )?;
      }
      Ok(())
   }

   /// Ensures the peer's selection is initialized.
   fn network_peer_activate(&mut self, _net: Net, peer_id: PeerId) -> netcanv::Result<()> {
      self.ensure_peer(peer_id);
      Ok(())
   }

   /// Ensures the peer's selection has been transferred to the paint canvas.
   fn network_peer_deactivate(
      &mut self,
      renderer: &mut Backend,
      _net: Net,
      paint_canvas: &mut PaintCanvas,
      peer_id: PeerId,
   ) -> netcanv::Result<()> {
      tracing::trace!("shape {:?} deactivated", peer_id);
      if let Some(peer) = self.peer_shapes.get_mut(&peer_id) {
         if let PeerState::Selection(selection) = &mut peer.state {
            selection.deselect(renderer, paint_canvas);
         }
      }
      Ok(())
   }
}

#[derive(Debug)]
struct Selection {
   rect: Option<Rect>,
   shape: Shape,
   color: Color,
   deselected_at: Option<Rect>,
}

impl Selection {
   const MAX_SIZE: u32 = 1024;

   fn with_normalized_rect(&self) -> Self {
      Self {
         rect: self.normalized_rect(),
         shape: self.shape,
         color: self.color,
         deselected_at: self.deselected_at,
      }
   }

   /// Cancels the selection, without transferring it to a paint canvas.
   fn cancel(&mut self) {
      self.rect = None;
      self.shape = Shape::Rectangle;
   }

   /// Finishes the selection, drawing the shape onto the given paint canvas.
   fn deselect(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      self.deselected_at = self.rect;
      if let Some(rect) = self.normalized_rect() {
         paint_canvas.draw(renderer, rect, |renderer| {
            draw_shape(renderer, self.shape, rect, self.color);
         });
      }
      self.cancel();
   }

   /// Returns a rounded, limited version of the rectangle.
   fn normalized_rect(&self) -> Option<Rect> {
      normalize_rect(&self.rect)
   }
}

enum PeerState {
   Shape {
      shape: Shape,
      rect: Option<Rect>,
      color: Color,
   },
   Selection(Selection),
}

impl PeerState {
   fn normalized_rect(&self) -> Option<Rect> {
      match self {
         Self::Shape { rect, .. } => normalize_rect(&rect),
         PeerState::Selection(selection) => selection.normalized_rect(),
      }
   }

   fn set_rect(&mut self, rect: Rect) {
      match self {
         Self::Shape { rect: srect, .. } => *srect = Some(rect),
         Self::Selection(selection) => selection.rect = Some(rect),
      }
   }

   fn set_color(&mut self, color: Color) {
      match self {
         Self::Shape { color: scolor, .. } => *scolor = color,
         Self::Selection(selection) => selection.color = color,
      }
   }

   fn set_shape(&mut self, shape: Shape) {
      match self {
         Self::Shape { shape: sshape, .. } => *sshape = shape,
         Self::Selection(selection) => selection.shape = shape,
      }
   }
}

struct PeerShape {
   state: PeerState,
   previous_normalized_rect: Option<Rect>,
   last_rect_packet: Instant,
   mouse_position: Point,
   previous_mouse_position: Point,
   cursor_color: Color,
   last_cursor_packet: Instant,
}

impl PeerShape {
   fn lerp_normalized_rect(&self) -> Option<Rect> {
      let elapsed = self.last_rect_packet.elapsed().as_millis() as f32;
      let t = (elapsed / paint::State::TIME_PER_UPDATE.as_millis() as f32).clamp(0.0, 1.0);
      self.state.normalized_rect().map(|mut rect| {
         let previous_rect = self.previous_normalized_rect.unwrap_or(rect);
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

/// A network packet for the shape tool.
#[derive(Serialize, Deserialize)]
enum Packet {
   /// The rectangle and color.
   Update {
      position: (f32, f32),
      size: (f32, f32),
      color: (u8, u8, u8, u8),
   },
   /// Turn shape into a selection.
   Selection,
   /// Invoke [`Selection::cancel`].
   Cancel,
   /// Invoke [`Selection::deselect`].
   Deselect,
   /// Change shape.
   Shape(Shape),
   /// Cursor's position and color.
   Cursor {
      position: (f32, f32),
      color: (u8, u8, u8, u8),
   },
}
