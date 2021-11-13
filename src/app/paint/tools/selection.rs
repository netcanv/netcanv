use netcanv_renderer::paws::{point, vector, Color, Point, Rect, Renderer, Vector};
use netcanv_renderer::{BlendMode, Font as FontTrait, RenderBackend};
use winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::{Backend, Font, Framebuffer, Image};
use crate::common::{RectMath, VectorMath};
use crate::paint_canvas::PaintCanvas;
use crate::ui::{UiElements, UiInput};
use crate::viewport::Viewport;

use super::{Tool, ToolArgs};

struct Icons {
   tool: Image,
   cursor: Image,
   position: Image,
   rectangle: Image,
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
   Selecting,
   DraggingHandle(Handle),
   DraggingWhole,
}

pub struct SelectionTool {
   icons: Icons,
   mouse_position: Point,
   /// The "potential" action; that is, the action that can be triggered right now by left-clicking.
   potential_action: Action,
   action: Action,
   selection: Selection,
}

impl SelectionTool {
   const COLOR: Color = Color::rgb(0x0397fb);
   const HANDLE_RADIUS: f32 = 4.0;

   pub fn new() -> Self {
      Self {
         icons: Icons {
            tool: Assets::load_icon(include_bytes!("../../../assets/icons/selection.svg")),
            cursor: Assets::load_icon(include_bytes!("../../../assets/icons/position.svg")),
            position: Assets::load_icon(include_bytes!(
               "../../../assets/icons/selection-position.svg"
            )),
            rectangle: Assets::load_icon(include_bytes!(
               "../../../assets/icons/selection-rectangle.svg"
            )),
         },
         mouse_position: point(0.0, 0.0),
         potential_action: Action::None,
         action: Action::None,
         selection: Selection {
            rect: None,
            capture: None,
         },
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

   fn rect_is_smaller_than_a_pixel(rect: Rect) -> bool {
      rect.width().trunc().abs() < 1.0 || rect.height().trunc().abs() < 1.0
   }
}

impl Tool for SelectionTool {
   fn name(&self) -> &str {
      "Selection"
   }

   fn icon(&self) -> &Image {
      &self.icons.tool
   }

   fn deactivate(&mut self, renderer: &mut Backend, paint_canvas: &mut PaintCanvas) {
      self.selection.deselect(renderer, paint_canvas);
   }

   fn process_paint_canvas_input(
      &mut self,
      ToolArgs { ui, input, .. }: ToolArgs,
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
               );
               if mouse_position.is_in_rect(rect) {
                  self.potential_action = Action::DraggingWhole;
               }
            }
         }
      }

      // Check if the left mouse button was pressed, and if so, start selecting.
      if input.mouse_button_just_pressed(MouseButton::Left) {
         if self.potential_action == Action::Selecting {
            // Before we erase the old data, draw the capture back onto the canvas.
            self.selection.deselect(ui, paint_canvas);
            // Anchor the selection to the mouse position.
            self.selection.begin(mouse_position);
         }
         self.action = self.potential_action;
      }
      if input.mouse_button_just_released(MouseButton::Left) {
         // After the button is released and the selection's size is close to 0, deselect.
         if let Some(rect) = self.selection.rect {
            if Self::rect_is_smaller_than_a_pixel(rect) {
               self.selection.cancel();
            }
         }
         if self.action == Action::Selecting {
            // Normalize the stored selection after the user's done marking.
            // This will make sure that before making any other actions mutating the selection, the
            // selection's rectangle satisfies all the expectations, eg. that the corners' names are
            // what they are visually.
            self.selection.normalize();
            // If there's still a selection after all of this, capture the paint canvas into an
            // image.
            self.selection.capture(ui, paint_canvas);
         }
         self.action = Action::None;
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
}

struct Selection {
   rect: Option<Rect>,
   capture: Option<Framebuffer>,
}

impl Selection {
   const MAX_SIZE: f32 = 1024.0;

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
      self.rect = None;
      self.capture = None;
   }

   /// Returns a sorted, rounded, limited, squared version of the selection rectangle.
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

fn format_vector(vector: Vector) -> String {
   format!("{:.0}, {:.0}", vector.x, vector.y)
}

fn label_width(font: &Font, text: &str) -> f32 {
   font.text_width(text).max(96.0)
}
