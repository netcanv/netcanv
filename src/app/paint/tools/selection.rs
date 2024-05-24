use image::imageops::FilterType;
use instant::Instant;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use tokio::sync::{mpsc, oneshot};

use crate::backend::winit::event::MouseButton;
use crate::backend::winit::window::CursorIcon;
use crate::config::config;
use crate::keymap::KeyBinding;
use image::codecs::png::PngEncoder;
use image::io::Reader;
use image::{ColorType, ImageEncoder, ImageFormat, RgbaImage};
use netcanv_protocol::relay::PeerId;
use netcanv_renderer::paws::{point, vector, AlignH, AlignV, Color, Point, Rect, Renderer, Vector};
use netcanv_renderer::{
   BlendMode, Font as FontTrait, Framebuffer as FramebufferTrait, RenderBackend,
};
use serde::{Deserialize, Serialize};

use crate::app::paint::{self, GlobalControls};
use crate::assets::Assets;
use crate::backend::{Backend, Font, Framebuffer, Image};
use crate::clipboard;
use crate::common::{deserialize_bincode, lerp_point, RectMath, VectorMath};
use crate::paint_canvas::PaintCanvas;
use crate::ui::{ButtonState, UiElements, UiInput};
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

impl Action {
   fn or(self, rhs: Self) -> Self {
      if self == Self::None {
         rhs
      } else {
         self
      }
   }
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

   paste: Option<(
      Point,
      oneshot::Receiver<RgbaImage>,
      oneshot::Receiver<Vec<u8>>,
   )>,
   peer_pastes_tx: mpsc::UnboundedSender<(PeerId, Option<RgbaImage>)>,
   peer_pastes_rx: mpsc::UnboundedReceiver<(PeerId, Option<RgbaImage>)>,
   ongoing_paste_jobs: HashSet<PeerId>,
}

impl SelectionTool {
   /// The color of the selection.
   const COLOR: Color = Color::rgb(0x0397fb);
   /// The radius of handles for resizing the selection contents.
   const HANDLE_RADIUS: f32 = 4.0;

   pub fn new(renderer: &mut Backend) -> Self {
      let (peer_pastes_tx, peer_pastes_rx) = mpsc::unbounded_channel();
      Self {
         icons: Icons {
            tool: Assets::load_svg(
               renderer,
               include_bytes!("../../../assets/icons/selection.svg"),
            ),
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
               include_bytes!("../../../assets/icons/selection-rectangle.svg"),
            ),
         },
         mouse_position: point(0.0, 0.0),
         potential_action: Action::None,
         action: Action::None,
         selection: Selection::new(),
         peer_selections: HashMap::new(),

         paste: None,
         peer_pastes_tx,
         peer_pastes_rx,
         ongoing_paste_jobs: HashSet::new(),
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
      renderer.fill_circle(position, radius, Self::COLOR);
   }

   /// Returns whether a rect is smaller than a pixel.
   fn rect_is_smaller_than_a_pixel(rect: Rect) -> bool {
      rect.width().trunc().abs() < 1.0 || rect.height().trunc().abs() < 1.0
   }

   /// Ensures that a peer's selection is properly initialized. Returns a mutable reference to
   /// said selection.
   fn ensure_peer(&mut self, peer_id: PeerId) -> &mut PeerSelection {
      self.peer_selections.entry(peer_id).or_insert(PeerSelection {
         selection: Selection::new(),
         previous_normalized_rect: None,
         last_rect_packet: Instant::now(),
      })
   }

   /// Sends a `Rect` packet containing the current selection rectangle.
   /// This is sometimes needed before important actions, where the rectangle may not have been
   /// synchronized yet due to the lower network tick rate.
   fn send_rect_packet(&self, net: &Net) -> netcanv::Result<()> {
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
   fn copy_to_clipboard(&self, renderer: &mut Backend) {
      if let Some(image) = self.selection.download_rgba(renderer) {
         catch!(clipboard::copy_image(image));
      }
   }

   /// Pastes the clipboard image into a new selection.
   fn enqueue_paste_from_clipboard(&mut self, position: Point) {
      let (image_tx, image_rx) = oneshot::channel();
      let (bytes_tx, bytes_rx) = oneshot::channel();
      self.paste = Some((position, image_rx, bytes_rx));
      tokio::task::spawn_blocking(|| {
         tracing::debug!("reading image from clipboard");
         let image = catch!(clipboard::paste_image());
         let image = if image.width() > Selection::MAX_SIZE || image.height() > Selection::MAX_SIZE
         {
            tracing::debug!("image is too big! scaling down");
            let scale = Selection::MAX_SIZE as f32 / image.width().max(image.height()) as f32;
            let new_width = (image.width() as f32 * scale) as u32;
            let new_height = (image.height() as f32 * scale) as u32;
            image::imageops::resize(&image, new_width, new_height, FilterType::Triangle)
         } else {
            image
         };
         // The result here doesn't matter. If the image doesn't arrive, we're out of the
         // paint state.
         let _ = image_tx.send(image.clone());
         tracing::debug!("encoding image for transmission");
         let bytes = catch!(Self::encode_image(&image));
         tracing::debug!("paste job done; encoded {} bytes", bytes.len());
         let _ = bytes_tx.send(bytes);
      });
   }

   /// Polls whether the paste operation is complete. Returns `true` when the tool should be
   /// switched to the selection tool.
   fn poll_paste_from_clipboard(
      &mut self,
      renderer: &mut Backend,
      paint_canvas: &mut PaintCanvas,
      net: &Net,
   ) -> bool {
      if let Some((position, image, bytes)) = self.paste.as_mut() {
         if let Ok(image) = image.try_recv() {
            self.selection.deselect(renderer, paint_canvas);
            self.selection.paste(renderer, Some(*position), &image);
            return true;
         }
         if let Ok(bytes) = bytes.try_recv() {
            let Point { x, y } = *position;
            catch!(
               net.send(self, PeerId::BROADCAST, Packet::Paste((x, y), bytes)),
               return false
            );
            catch!(self.send_rect_packet(net), return false);
            // Once the bytes have been encoded and sent to the other clients, there's no use in
            // keeping this data around anymore.
            self.paste = None;
         }
      }

      false
   }

   fn poll_peer_pastes(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      while let Ok((peer_id, image)) = self.peer_pastes_rx.try_recv() {
         let peer = self.ensure_peer(peer_id);
         let deselected_before_decoding_finished = peer.selection.rect.is_none();
         if !deselected_before_decoding_finished {
            peer.selection.deselect(renderer, paint_canvas);
         }
         if let Some(image) = image {
            // We don't update the rectangle here because a data race could happen.
            // The peer sends a rect packet immediately after the paste packet anyways.
            tracing::debug!("finishing peer paste");
            peer.selection.paste(renderer, None, &image);
            if deselected_before_decoding_finished {
               tracing::debug!("the peer deselected before decoding had a chance to finish");
               peer.selection.rect = peer.selection.deselected_at;
               peer.selection.deselect(renderer, paint_canvas);
            }
         }
         self.ongoing_paste_jobs.remove(&peer_id);
      }
   }

   /// Encodes an image to PNG.
   fn encode_image(image: &RgbaImage) -> netcanv::Result<Vec<u8>> {
      let mut bytes = Vec::new();
      PngEncoder::new(Cursor::new(&mut bytes)).write_image(
         image,
         image.width(),
         image.height(),
         ColorType::Rgba8,
      )?;
      Ok(bytes)
   }

   /// Decodes a PNG image.
   fn decode_image(data: &[u8]) -> netcanv::Result<RgbaImage> {
      Ok(Reader::with_format(Cursor::new(data), ImageFormat::Png).decode()?.to_rgba8())
   }
}

impl Tool for SelectionTool {
   fn name(&self) -> &'static str {
      "selection"
   }

   fn icon(&self) -> &Image {
      &self.icons.tool
   }

   fn key_shortcut(&self) -> KeyBinding {
      config().keymap.tools.selection.clone()
   }

   /// When the tool is deactivated, the selection should be deselected.
   fn deactivate(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      self.selection.deselect(renderer, paint_canvas);
   }

   /// Processes key shortcuts when the selection is active.
   fn active_key_shortcuts(
      &mut self,
      ToolArgs { input, net, ui, .. }: ToolArgs,
      _paint_canvas: &mut PaintCanvas,
      _viewport: &Viewport,
   ) -> KeyShortcutAction {
      if input.action(&config().keymap.edit.delete) == (true, true) {
         if self.selection.rect.is_some() {
            self.selection.cancel();
            catch!(
               net.send(self, PeerId::BROADCAST, Packet::Cancel),
               return KeyShortcutAction::None
            );
         }
         return KeyShortcutAction::Success;
      }

      if input.action(&config().keymap.edit.copy) == (true, true) {
         self.copy_to_clipboard(ui);
         return KeyShortcutAction::Success;
      }

      if input.action(&config().keymap.edit.cut) == (true, true) {
         self.copy_to_clipboard(ui);
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
      if input.action(&config().keymap.edit.paste) == (true, true) {
         tracing::info!("pasting image from clipboard");
         self.enqueue_paste_from_clipboard(viewport.pan());
      }

      if self.poll_paste_from_clipboard(ui, paint_canvas, &net) {
         return KeyShortcutAction::SwitchToThisTool;
      }

      KeyShortcutAction::None
   }

   fn process_background_jobs(
      &mut self,
      ToolArgs { ui, .. }: ToolArgs,
      paint_canvas: &mut PaintCanvas,
   ) {
      self.poll_peer_pastes(ui.render(), paint_canvas);
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

      let handle_radius = Self::HANDLE_RADIUS * 3.0 / viewport.zoom();
      self.potential_action = Action::Selecting;
      // Only let the user resize or drag the selection if they aren't doing anything at the moment.
      if matches!(self.action, Action::None | Action::DraggingWhole) {
         if let Some(rect) = self.selection.rect {
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
         Action::Selecting => CursorIcon::Crosshair,
         Action::DraggingHandle(_) => {
            // We process the hovered handles for a second time, because the first time around the
            // rectangle was not sorted.
            if let Some(rect) = self.selection.normalized_rect() {
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
                  renderer.framebuffer(rect, capture);
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
      ui.horizontal_label(
         &assets.sans,
         &mouse_position,
         assets.colors.text,
         Some((label_width(&assets.sans, &mouse_position), AlignH::Center)),
      );

      if let Some(rect) = self.selection.normalized_rect() {
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

   /// Sends out packets containing the selection rectangle.
   fn network_send(&mut self, net: Net, _: &GlobalControls) -> netcanv::Result<()> {
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
   ) -> netcanv::Result<()> {
      let packet = deserialize_bincode(&payload)?;
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
                  width.min(Selection::MAX_SIZE as f32),
                  height.min(Selection::MAX_SIZE as f32),
               ),
            ));
            peer.last_rect_packet = Instant::now();
         }
         Packet::Capture => peer.selection.capture(renderer, paint_canvas),
         Packet::Cancel => peer.selection.cancel(),
         Packet::Deselect => peer.selection.deselect(renderer, paint_canvas),
         Packet::Paste((_x, _y), data) => {
            // ↑ (x, y) is only here for compatibility with 0.7.0 and is no longer used
            // because it caused a data race
            tracing::debug!("{} pasted image ({} bytes of data)", sender, data.len());
            let tx = self.peer_pastes_tx.clone();
            self.ongoing_paste_jobs.insert(sender);
            tokio::task::spawn_blocking(move || match Self::decode_image(&data) {
               Ok(image) => {
                  let _ = tx.send((sender, Some(image)));
               }
               Err(error) => {
                  tracing::error!("could not decode selection image: {:?}", error);
                  let _ = tx.send((sender, None));
               }
            });
         }
         Packet::Update(data) => peer.selection.upload_rgba(renderer, &Self::decode_image(&data)?),
      }
      Ok(())
   }

   /// Sends a capture packet to the peer that joined.
   fn network_peer_join(
      &mut self,
      renderer: &mut Backend,
      net: Net,
      peer_id: PeerId,
   ) -> netcanv::Result<()> {
      if let Some(capture) = self.selection.download_rgba(renderer) {
         self.send_rect_packet(&net)?;
         net.send(self, peer_id, Packet::Update(Self::encode_image(&capture)?))?;
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
      tracing::debug!("selection {:?} deactivated", peer_id);
      if let Some(peer) = self.peer_selections.get_mut(&peer_id) {
         peer.selection.deselect(renderer, paint_canvas);
      }
      Ok(())
   }
}

struct Selection {
   rect: Option<Rect>,
   capture: Option<Framebuffer>,
   deselected_at: Option<Rect>,
}

impl std::fmt::Debug for Selection {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("Selection")
         .field("rect", &self.rect)
         .field("deselected_at", &self.deselected_at)
         .finish_non_exhaustive()
   }
}

impl Selection {
   const MAX_SIZE: u32 = 1024;

   fn new() -> Self {
      Self {
         rect: None,
         capture: None,
         deselected_at: None,
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
            renderer.set_blend_mode(BlendMode::Replace);
            renderer.fill(rect, Color::TRANSPARENT, 0.0);
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
      self.deselected_at = self.rect;
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
   fn download_rgba(&self, renderer: &mut Backend) -> Option<RgbaImage> {
      if let Some(rect) = self.normalized_rect() {
         let rect = rect.sort();
         if let Some(capture) = self.capture.as_ref() {
            let mut image = RgbaImage::new(rect.width() as u32, rect.height() as u32);
            renderer.download_framebuffer(capture, (0, 0), capture.size(), &mut image);
            return Some(image);
         }
      }
      None
   }

   /// Uploads the given image into the capture framebuffer.
   /// Does not do anything else with the selection; the rectangle must be initialized separately.
   fn upload_rgba(&mut self, renderer: &mut Backend, image: &RgbaImage) {
      let capture = renderer.create_framebuffer(image.width(), image.height());
      renderer.upload_framebuffer(&capture, (0, 0), (image.width(), image.height()), image);
      self.capture = Some(capture);
   }

   /// Creates a new selection with the given image capture, at the given origin.
   fn paste(&mut self, renderer: &mut Backend, position: Option<Point>, image: &RgbaImage) {
      if let Some(position) = position {
         let rect = Rect::new(
            position,
            vector(image.width() as f32, image.height() as f32),
         );

         // Limit the rectangle to a maximum width (or height) of 1024.
         // These calculations are performed in order to make the shorter dimension scaled
         // proportionally to the shorter one.
         let long_side = rect.width().max(rect.height());
         let scale = long_side.min(Self::MAX_SIZE as f32) / long_side;
         let rect = Rect::new(rect.position, rect.size * scale);

         // Center the rectangle on the screen.
         let rect = Rect::new(rect.position - rect.size / 2.0, rect.size);
         self.rect = Some(rect);
         self.normalize();
      }
      self.upload_rgba(renderer, image);
   }

   /// Returns a rounded, limited version of the selection rectangle.
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
