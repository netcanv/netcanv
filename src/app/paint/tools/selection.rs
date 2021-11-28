use std::collections::HashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::time::Instant;

use crate::backend::winit::event::{MouseButton, VirtualKeyCode};
use image::io::Reader;
use image::png::PngEncoder;
use image::{ColorType, ImageFormat, RgbaImage};
use netcanv_protocol::matchmaker::PeerId;
use netcanv_renderer::paws::{point, vector, AlignH, AlignV, Color, Point, Rect, Renderer, Vector};
use netcanv_renderer::{
   BlendMode, Font as FontTrait, Framebuffer as FramebufferTrait, RenderBackend,
};
use serde::{Deserialize, Serialize};

use crate::app::paint;
use crate::assets::Assets;
use crate::backend::{Backend, Font, Framebuffer, Image};
use crate::clipboard;
use crate::common::{lerp_point, RectMath, VectorMath};
use crate::paint_canvas::PaintCanvas;
use crate::ui::{ButtonState, Modifier, UiElements, UiInput};
use crate::viewport::Viewport;

use super::{KeyShortcutAction, Net, Tool, ToolArgs};

/// The icon set for the selection tool.
struct Icons {
   tool: Image,
   cursor: Image,
   position: Image,
   rectangle: Image,
}

/// Resizing handles.
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

/// An (inter)action that can be performed on the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
   None,
   Selecting,
   DraggingHandle(Handle),
   DraggingWhole,
}

/// The selection tool.
pub struct SelectionTool {
   icons: Icons,
   mouse_position: Point,
   /// The "potential" action; that is, the action that can be triggered right now by left-clicking.
   potential_action: Action,
   action: Action,
   selection: Selection,
   peer_selections: HashMap<PeerId, PeerSelection>,
}

impl SelectionTool {
   /// The color of the selection.
   const COLOR: Color = Color::rgb(0x0397fb);
   /// The radius of handles for resizing the selection contents.
   const HANDLE_RADIUS: f32 = 4.0;

   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icons: Icons {
            tool: Assets::load_icon(
               renderer,
               include_bytes!("../../../assets/icons/selection.svg"),
            ),
            cursor: Assets::load_icon(
               renderer,
               include_bytes!("../../../assets/icons/position.svg"),
            ),
            position: Assets::load_icon(
               renderer,
               include_bytes!("../../../assets/icons/selection-position.svg"),
            ),
            rectangle: Assets::load_icon(
               renderer,
               include_bytes!("../../../assets/icons/selection-rectangle.svg"),
            ),
         },
         mouse_position: point(0.0, 0.0),
         potential_action: Action::None,
         action: Action::None,
         selection: Selection::new(),
         peer_selections: HashMap::new(),
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
      renderer.fill_circle(position, radius, Self::COLOR);
   }

   /// Returns whether a rect is smaller than a pixel.
   fn rect_is_smaller_than_a_pixel(rect: Rect) -> bool {
      rect.width().trunc().abs() < 1.0 || rect.height().trunc().abs() < 1.0
   }

   /// Ensures that a peer's selection is properly initialized. Returns a mutable reference to
   /// said selection.
   fn ensure_peer(&mut self, peer_id: PeerId) -> &mut PeerSelection {
      if !self.peer_selections.contains_key(&peer_id) {
         self.peer_selections.insert(
            peer_id,
            PeerSelection {
               selection: Selection::new(),
               previous_normalized_rect: None,
               last_rect_packet: Instant::now(),
            },
         );
      }
      self.peer_selections.get_mut(&peer_id).unwrap()
   }

   /// Sends a `Rect` packet containing the current selection rectangle.
   /// This is sometimes needed before important actions, where the rectangle may not have been
   /// synchronized yet due to the lower network tick rate.
   fn send_rect_packet(&self, net: &Net) -> anyhow::Result<()> {
      if let Some(rect) = self.selection.normalized_rect() {
         net.send(
            self,
            PeerId::BROADCAST,
            Packet::Rect {
               position: (rect.x(), rect.y()),
               size: (rect.width(), rect.height()),
            },
         )?;
      }
      Ok(())
   }

   /// Copies the current selection to the system clipboard.
   fn copy_to_clipboard(&self) {
      if let Some(image) = self.selection.download_rgba() {
         catch!(clipboard::copy_image(image));
      }
   }

   /// Pastes the clipboard image into a new selection.
   fn paste_from_clipboard(
      &mut self,
      renderer: &mut Backend,
      paint_canvas: &mut PaintCanvas,
      net: &Net,
      position: Point,
   ) {
      let image = catch!(clipboard::paste_image());
      self.selection.deselect(renderer, paint_canvas);
      self.selection.paste(renderer, position, &image);

      let bytes = catch!(Self::encode_image(image));
      let Point { x, y } = position;
      catch!(net.send(self, PeerId::BROADCAST, Packet::Paste((x, y), bytes)));
      catch!(self.send_rect_packet(net));
   }

   /// Encodes an image to PNG.
   fn encode_image(image: RgbaImage) -> anyhow::Result<Vec<u8>> {
      let mut bytes = Vec::new();
      PngEncoder::new(Cursor::new(&mut bytes)).encode(
         &image,
         image.width(),
         image.height(),
         ColorType::Rgba8,
      )?;
      Ok(bytes)
   }

   /// Decodes a PNG image.
   fn decode_image(data: &[u8]) -> anyhow::Result<RgbaImage> {
      Ok(Reader::with_format(Cursor::new(data), ImageFormat::Png).decode()?.to_rgba8())
   }
}

impl Tool for SelectionTool {
   fn name(&self) -> &str {
      "Selection"
   }

   fn icon(&self) -> &Image {
      &self.icons.tool
   }

   /// When the tool is deactivated, the selection should be deselected.
   fn deactivate(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      self.selection.deselect(renderer, paint_canvas);
   }

   /// Processes key shortcuts when the selection is active.
   fn active_key_shortcuts(
      &mut self,
      ToolArgs { input, net, .. }: ToolArgs,
      _paint_canvas: &mut PaintCanvas,
      _viewport: &Viewport,
   ) -> KeyShortcutAction {
      if input.action((Modifier::NONE, VirtualKeyCode::Delete)) == (true, true) {
         if self.selection.rect.is_some() {
            self.selection.cancel();
            catch!(
               net.send(self, PeerId::BROADCAST, Packet::Cancel),
               return KeyShortcutAction::None
            );
         }
         return KeyShortcutAction::Success;
      }

      if input.action((Modifier::CTRL, VirtualKeyCode::C)) == (true, true) {
         self.copy_to_clipboard();
         return KeyShortcutAction::Success;
      }

      if input.action((Modifier::CTRL, VirtualKeyCode::X)) == (true, true) {
         self.copy_to_clipboard();
         self.selection.cancel();
         return KeyShortcutAction::Success;
      }

      KeyShortcutAction::None
   }

   /// Processes the global key shortcuts for the selection.
   fn global_key_shortcuts(
      &mut self,
      ToolArgs { ui, input, net, .. }: ToolArgs,
      paint_canvas: &mut PaintCanvas,
      viewport: &Viewport,
   ) -> KeyShortcutAction {
      if input.action((Modifier::CTRL, VirtualKeyCode::V)) == (true, true) {
         self.paste_from_clipboard(ui, paint_canvas, &net, viewport.pan());
         return KeyShortcutAction::SwitchToThisTool;
      }

      KeyShortcutAction::None
   }

   /// Processes mouse input.
   fn process_paint_canvas_input(
      &mut self,
      ToolArgs { ui, input, net, .. }: ToolArgs,
      paint_canvas: &mut PaintCanvas,
      viewport: &Viewport,
   ) {
      // Calculate the mouse position.
      let mouse_position = ui.mouse_position(input);
      let mouse_position = viewport.to_viewport_space(mouse_position, ui.size());
      let previous_mouse_position = ui.previous_mouse_position(input);
      let previous_mouse_position = viewport.to_viewport_space(previous_mouse_position, ui.size());
      // Store the mouse position for the bottom bar display.
      self.mouse_position = mouse_position;

      self.potential_action = Action::Selecting;
      // Only let the user resize or drag the selection if they aren't doing anything at the moment.
      if matches!(self.action, Action::None | Action::DraggingWhole) {
         if let Some(rect) = self.selection.rect {
            // Check the handles.
            let handle_radius = Self::HANDLE_RADIUS * 3.0 / viewport.zoom();
            let handle = if mouse_position.is_in_circle(rect.top_left(), handle_radius) {
               Some(Handle::TopLeft)
            } else if mouse_position.is_in_circle(rect.top_center(), handle_radius) {
               Some(Handle::Top)
            } else if mouse_position.is_in_circle(rect.top_right(), handle_radius) {
               Some(Handle::TopRight)
            } else if mouse_position.is_in_circle(rect.right_center(), handle_radius) {
               Some(Handle::Right)
            } else if mouse_position.is_in_circle(rect.bottom_right(), handle_radius) {
               Some(Handle::BottomRight)
            } else if mouse_position.is_in_circle(rect.bottom_center(), handle_radius) {
               Some(Handle::Bottom)
            } else if mouse_position.is_in_circle(rect.bottom_left(), handle_radius) {
               Some(Handle::BottomLeft)
            } else if mouse_position.is_in_circle(rect.left_center(), handle_radius) {
               Some(Handle::Left)
            } else {
               None
            };
            if let Some(handle) = handle {
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

      // Check if the left mouse button was pressed, and if so, start selecting.
      match input.action(MouseButton::Left) {
         (true, ButtonState::Pressed) => {
            if self.potential_action == Action::Selecting {
               // Before we erase the old data, draw the capture back onto the canvas.
               self.selection.deselect(ui, paint_canvas);
               catch!(self.send_rect_packet(&net));
               catch!(net.send(self, PeerId::BROADCAST, Packet::Deselect));
               // Anchor the selection to the mouse position.
               self.selection.begin(mouse_position);
               catch!(self.send_rect_packet(&net));
            }
            self.action = self.potential_action;
         }
         (_, ButtonState::Released) => {
            // After the button is released and the selection's size is close to 0, deselect.
            if let Some(rect) = self.selection.rect {
               if Self::rect_is_smaller_than_a_pixel(rect) {
                  self.selection.cancel();
                  catch!(net.send(self, PeerId::BROADCAST, Packet::Cancel));
               }
            }
            if self.action == Action::Selecting {
               // Normalize the stored selection after the user's done marking.
               // This will make sure that before making any other actions mutating the selection,
               // the selection's rectangle satisfies all the expectations, eg. that the corners'
               // names are what they are visually.
               self.selection.normalize();
               catch!(self.send_rect_packet(&net));
               // If there's still a selection after all of this, capture the paint canvas into an
               // image.
               self.selection.capture(ui, paint_canvas);
               catch!(net.send(self, PeerId::BROADCAST, Packet::Capture));
            }
            self.action = Action::None;
         }
         _ => (),
      }

      // Perform all the actions.
      if let Some(rect) = self.selection.rect.as_mut() {
         match self.action {
            Action::None => (),
            Action::Selecting => {
               rect.size = mouse_position - rect.position;
            }
            Action::DraggingHandle(handle) => {
               match handle {
                  Handle::TopLeft => *rect = rect.with_top_left(mouse_position),
                  Handle::Top => *rect = rect.with_top(mouse_position.y),
                  Handle::TopRight => *rect = rect.with_top_right(mouse_position),
                  Handle::Right => *rect = rect.with_right(mouse_position.x),
                  Handle::BottomRight => *rect = rect.with_bottom_right(mouse_position),
                  Handle::Bottom => *rect = rect.with_bottom(mouse_position.y),
                  Handle::BottomLeft => *rect = rect.with_bottom_left(mouse_position),
                  Handle::Left => *rect = rect.with_left(mouse_position.x),
               }
               self.selection.rect = self.selection.normalized_rect();
            }
            Action::DraggingWhole => {
               let delta_position = mouse_position - previous_mouse_position;
               rect.position += delta_position;
            }
         }
      }
   }

   /// Processes the selection overlay.
   fn process_paint_canvas_overlays(&mut self, ToolArgs { ui, .. }: ToolArgs, viewport: &Viewport) {
      if let Some(rect) = self.selection.normalized_rect() {
         if !Self::rect_is_smaller_than_a_pixel(rect) {
            ui.draw(|ui| {
               // Oh my.
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
               if let Some(capture) = self.selection.capture.as_ref() {
                  renderer.framebuffer(rect, &capture);
               }
               renderer.outline(
                  rect,
                  Self::COLOR,
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

   /// Processes peers' selection overlays.
   fn process_paint_canvas_peer(
      &mut self,
      ToolArgs {
         ui, net, assets, ..
      }: ToolArgs,
      viewport: &Viewport,
      peer_id: PeerId,
   ) {
      if let Some(peer) = self.peer_selections.get(&peer_id) {
         if let Some(rect) = peer.lerp_normalized_rect() {
            if !Self::rect_is_smaller_than_a_pixel(rect) {
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
                  if let Some(framebuffer) = peer.selection.capture.as_ref() {
                     renderer.framebuffer(rect, framebuffer);
                  }
                  renderer.outline(rect, Self::COLOR, 0.0, 2.0);
                  if rect.width() > text_rect.width() && rect.height() > text_rect.height() {
                     renderer.fill(text_rect, Self::COLOR, 2.0);
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

   /// Processes the bottom bar stats.
   fn process_bottom_bar(&mut self, ToolArgs { ui, assets, .. }: ToolArgs) {
      let icon_size = vector(ui.height(), ui.height());

      // Show the mouse position.
      let mouse_position = format_vector(self.mouse_position);
      ui.icon(&self.icons.cursor, assets.colors.text, Some(icon_size));
      ui.label(
         &assets.sans,
         &mouse_position,
         assets.colors.text,
         Some(label_width(&assets.sans, &mouse_position)),
      );

      if let Some(rect) = self.selection.normalized_rect() {
         let rect = rect.sort();
         // Show the selection anchor.
         let anchor = format_vector(rect.position);
         ui.icon(&self.icons.position, assets.colors.text, Some(icon_size));
         ui.label(
            &assets.sans,
            &anchor,
            assets.colors.text,
            Some(label_width(&assets.sans, &anchor)),
         );
         let size = format!("{:.0} × {:.0}", rect.width(), rect.height());
         ui.icon(&self.icons.rectangle, assets.colors.text, Some(icon_size));
         ui.label(
            &assets.sans,
            &size,
            assets.colors.text,
            Some(label_width(&assets.sans, &size)),
         );
      }
   }

   /// Sends out packets containing the selection rectangle.
   fn network_send(&mut self, net: Net) -> anyhow::Result<()> {
      self.send_rect_packet(&net)?;
      Ok(())
   }

   /// Interprets an incoming packet.
   fn network_receive(
      &mut self,
      renderer: &mut Backend,
      _net: Net,
      paint_canvas: &mut PaintCanvas,
      sender: PeerId,
      payload: Vec<u8>,
   ) -> anyhow::Result<()> {
      let packet = bincode::deserialize(&payload)?;
      let peer = self.ensure_peer(sender);
      match packet {
         Packet::Rect {
            position: (x, y),
            size: (width, height),
         } => {
            peer.previous_normalized_rect = peer.selection.normalized_rect();
            peer.selection.rect = Some(Rect::new(
               point(x, y),
               vector(
                  width.min(Selection::MAX_SIZE),
                  height.min(Selection::MAX_SIZE),
               ),
            ));
            peer.last_rect_packet = Instant::now();
         }
         Packet::Capture => peer.selection.capture(renderer, paint_canvas),
         Packet::Cancel => peer.selection.cancel(),
         Packet::Deselect => peer.selection.deselect(renderer, paint_canvas),
         Packet::Paste((x, y), data) => {
            peer.selection.deselect(renderer, paint_canvas);
            peer.selection.paste(renderer, point(x, y), &Self::decode_image(&data)?);
         }
         Packet::Update(data) => peer.selection.upload_rgba(renderer, &Self::decode_image(&data)?),
      }
      Ok(())
   }

   /// Sends a capture packet to the peer that joined.
   fn network_peer_join(&mut self, net: Net, peer_id: PeerId) -> anyhow::Result<()> {
      if let Some(capture) = self.selection.download_rgba() {
         self.send_rect_packet(&net)?;
         net.send(self, peer_id, Packet::Update(Self::encode_image(capture)?))?;
      }
      Ok(())
   }

   /// Ensures the peer's selection is initialized.
   fn network_peer_activate(&mut self, _net: Net, peer_id: PeerId) -> anyhow::Result<()> {
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
   ) -> anyhow::Result<()> {
      println!("selection {:?} deactivate", peer_id);
      if let Some(peer) = self.peer_selections.get_mut(&peer_id) {
         peer.selection.deselect(renderer, paint_canvas);
      }
      Ok(())
   }
}

struct Selection {
   rect: Option<Rect>,
   capture: Option<Framebuffer>,
}

impl Selection {
   const MAX_SIZE: f32 = 1024.0;

   fn new() -> Self {
      Self {
         rect: None,
         capture: None,
      }
   }

   /// Begins the selection at the given anchor.
   fn begin(&mut self, anchor: Point) {
      self.rect = Some(Rect::new(anchor, vector(0.0, 0.0)));
      self.rect = self.normalized_rect();
   }

   /// Captures the selection into a framebuffer. Clears the captured part of the selection from the
   /// paint canvas.
   fn capture(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      if let Some(rect) = self.rect {
         let viewport = Viewport::from_top_left(rect);
         let capture = renderer.create_framebuffer(rect.width() as u32, rect.height() as u32);
         renderer.push();
         renderer.translate(-rect.position);
         paint_canvas.capture(renderer, &capture, &viewport);
         renderer.pop();
         self.capture = Some(capture);
         // After the capture is taken, erase the rectangle from the paint canvas.
         paint_canvas.draw(renderer, rect, |renderer| {
            renderer.set_blend_mode(BlendMode::Clear);
            renderer.fill(rect, Color::BLACK, 0.0);
         });
      }
   }

   /// Cancels the selection, without transferring it to a paint canvas.
   fn cancel(&mut self) {
      self.rect = None;
      self.capture = None;
   }

   /// Finishes the selection, transferring the old rectangle to the given paint canvas.
   fn deselect(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      if let Some(capture) = self.capture.as_ref() {
         if let Some(rect) = self.normalized_rect() {
            paint_canvas.draw(renderer, rect, |renderer| {
               renderer.framebuffer(rect, capture);
            });
         }
      }
      self.cancel();
   }

   /// Downloads a captured selection off the graphics card, into an RGBA image.
   ///
   /// Returns `None` if there's no _captured_ selection.
   fn download_rgba(&self) -> Option<RgbaImage> {
      if let Some(rect) = self.normalized_rect() {
         let rect = rect.sort();
         if let Some(capture) = self.capture.as_ref() {
            let mut image = RgbaImage::new(rect.width() as u32, rect.height() as u32);
            capture.download_rgba(&mut image);
            return Some(image);
         }
      }
      None
   }

   /// Uploads the given image into the capture framebuffer.
   /// Does not do anything else with the selection; the rectangle must be initialized separately.
   fn upload_rgba(&mut self, renderer: &mut Backend, image: &RgbaImage) {
      let mut capture = renderer.create_framebuffer(image.width(), image.height());
      capture.upload_rgba((0, 0), (image.width(), image.height()), &image);
      self.capture = Some(capture);
   }

   /// Creates a new selection with the given image capture, at the given origin.
   fn paste(&mut self, renderer: &mut Backend, position: Point, image: &RgbaImage) {
      let rect = Rect::new(
         position,
         vector(image.width() as f32, image.height() as f32),
      );

      // Limit the rectangle to a maximum width (or height) of 1024.
      // These calculations are performed in order to make the shorter dimension scaled
      // proportionally to the shorter one.
      let long_side = rect.width().max(rect.height());
      let scale = long_side.min(Self::MAX_SIZE) / long_side;
      let rect = Rect::new(rect.position, rect.size * scale);

      // Center the rectangle on the screen.
      let rect = Rect::new(rect.position - rect.size / 2.0, rect.size);
      self.rect = Some(rect);
      self.normalize();

      self.upload_rgba(renderer, image);
   }

   /// Returns a rounded, limited version of the selection rectangle.
   fn normalized_rect(&self) -> Option<Rect> {
      self.rect.map(|rect| {
         let rect = Rect::new(
            rect.position,
            vector(
               rect.width().clamp(-Self::MAX_SIZE, Self::MAX_SIZE),
               rect.height().clamp(-Self::MAX_SIZE, Self::MAX_SIZE),
            ),
         );
         let rect = Rect::new(
            point(rect.x().floor(), rect.y().floor()),
            vector(rect.width().ceil(), rect.height().ceil()),
         );
         rect
      })
   }

   /// Normalizes the selection rectangle, such that the corner names match their visual positions.
   fn normalize(&mut self) {
      self.rect = self.normalized_rect().map(|rect| rect.sort());
   }
}

/// A peer's selection data.
struct PeerSelection {
   selection: Selection,
   previous_normalized_rect: Option<Rect>,
   last_rect_packet: Instant,
}

impl PeerSelection {
   fn lerp_normalized_rect(&self) -> Option<Rect> {
      let elapsed = self.last_rect_packet.elapsed().as_millis() as f32;
      let t = (elapsed / paint::State::TIME_PER_UPDATE.as_millis() as f32).clamp(0.0, 1.0);
      self.selection.normalized_rect().map(|mut rect| {
         let previous_rect = self.previous_normalized_rect.unwrap_or(rect);
         rect.position = lerp_point(previous_rect.position, rect.position, t);
         rect.size = lerp_point(previous_rect.size, rect.size, t);
         rect
      })
   }
}

/// A network packet for the selection tool.
#[derive(Serialize, Deserialize)]
enum Packet {
   /// The selection rectangle.
   Rect {
      position: (f32, f32),
      size: (f32, f32),
   },
   /// Invoke [`Selection::capture`].
   Capture,
   /// Invoke [`Selection::cancel`].
   Cancel,
   /// Invoke [`Selection::deselect`].
   Deselect,
   /// Paste an image at the provided origin, starting a new selection.
   Paste((f32, f32), Vec<u8>),
   /// Update the captured image.
   Update(Vec<u8>),
}

fn format_vector(vector: Vector) -> String {
   format!("{:.0}, {:.0}", vector.x, vector.y)
}

fn label_width(font: &Font, text: &str) -> f32 {
   font.text_width(text).max(96.0)
}
