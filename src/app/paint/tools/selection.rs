use netcanv_renderer::paws::{point, vector, Color, Point, Rect, Renderer, Vector};
use netcanv_renderer::{Font as FontTrait, RenderBackend};
use winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::{Backend, Font, Image};
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
enum Corner {
   TopLeft,
   TopRight,
   BottomRight,
   BottomLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
   None,
   Selecting,
   DraggingCorner(Corner),
   DraggingWhole,
}

pub struct Selection {
   icons: Icons,
   mouse_position: Point,
   /// The "potential" action; that is, the action that can be triggered right now by left-clicking.
   potential_action: Action,
   action: Action,
   selection: Option<Rect>,
}

impl Selection {
   const MAX_SIZE: f32 = 1024.0;
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
         selection: None,
      }
   }

   /// Returns a sorted, rounded, limited, squared version of the selection rectangle.
   fn selection(&self) -> Option<Rect> {
      self.selection.map(|rect| {
         let rect = Rect::new(
            rect.position,
            vector(
               rect.width().clamp(-Self::MAX_SIZE, Self::MAX_SIZE),
               rect.height().clamp(-Self::MAX_SIZE, Self::MAX_SIZE),
            ),
         );
         let rect = rect.sort();
         let rect = Rect::new(
            point(rect.x().floor(), rect.y().floor()),
            vector(rect.width().ceil(), rect.height().ceil()),
         );
         rect
      })
   }

   /// Draws a resize handle.
   fn draw_handle(&self, renderer: &mut Backend, position: Point, corner: Corner) {
      let radius = if self.potential_action == Action::DraggingCorner(corner) {
         Self::HANDLE_RADIUS * 2.0
      } else {
         Self::HANDLE_RADIUS
      };
      renderer.fill_circle(position, radius + 2.0, Color::WHITE);
      renderer.fill_circle(position, radius, Self::COLOR);
   }
}

impl Tool for Selection {
   fn name(&self) -> &str {
      "Selection"
   }

   fn icon(&self) -> &Image {
      &self.icons.tool
   }

   fn deactivate(&mut self) {
      self.selection = None;
   }

   fn process_paint_canvas_input(
      &mut self,
      ToolArgs { ui, input, .. }: ToolArgs,
      _paint_canvas: &mut PaintCanvas,
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
      if self.action == Action::None {
         if let Some(rect) = self.selection() {
            // Check the corners.
            let handle_radius = Self::HANDLE_RADIUS * 2.0 / viewport.zoom();
            let corner = if mouse_position.is_in_circle(rect.top_left(), handle_radius) {
               Some(Corner::TopLeft)
            } else if mouse_position.is_in_circle(rect.top_right(), handle_radius) {
               Some(Corner::TopRight)
            } else if mouse_position.is_in_circle(rect.bottom_right(), handle_radius) {
               Some(Corner::BottomRight)
            } else if mouse_position.is_in_circle(rect.bottom_left(), handle_radius) {
               Some(Corner::BottomLeft)
            } else {
               None
            };
            if let Some(corner) = corner {
               self.potential_action = Action::DraggingCorner(corner);
            } else {
               // Check the inside.
               if mouse_position.is_in_rect(rect) {
                  self.potential_action = Action::DraggingWhole;
               }
            }
         }
      }

      // Check if the left mouse button was pressed, and if yes, start selecting.
      if input.mouse_button_just_pressed(MouseButton::Left) {
         if self.potential_action == Action::Selecting {
            // Anchor the selection to the mouse position.
            self.selection = Some(Rect::new(mouse_position, vector(0.0, 0.0)));
         }
         self.action = self.potential_action;
      }
      if input.mouse_button_just_released(MouseButton::Left) {
         if self.action == Action::Selecting {
            // After the button is released and the selection's size is close to 0, deselect.
            if let Some(rect) = self.selection {
               if rect.width().abs() < 0.1 || rect.height().abs() < 0.1 {
                  self.selection = None;
               }
            }
         }
         // Normalize the stored selection after the user's done marking.
         self.selection = self.selection();
         self.action = Action::None;
      }

      // Perform all the actions.
      if let Some(rect) = self.selection.as_mut() {
         match self.action {
            Action::None => (),
            Action::Selecting => {
               // The rectangle must be Some while we're selecting.
               rect.size = mouse_position - rect.position;
            }
            Action::DraggingCorner(corner) => match corner {
               Corner::TopLeft => *rect = rect.with_top_left(mouse_position),
               Corner::TopRight => *rect = rect.with_top_right(mouse_position),
               Corner::BottomRight => *rect = rect.with_bottom_right(mouse_position),
               Corner::BottomLeft => *rect = rect.with_bottom_left(mouse_position),
            },
            Action::DraggingWhole => {
               let delta_position = mouse_position - previous_mouse_position;
               rect.position += delta_position;
            }
         }
      }
   }

   fn process_paint_canvas_overlays(&mut self, ToolArgs { ui, .. }: ToolArgs, viewport: &Viewport) {
      if let Some(rect) = self.selection() {
         if rect.width() * rect.height() > 0.1 {
            ui.draw(|ui| {
               let top_left = viewport.to_screen_space(rect.top_left(), ui.size()).floor();
               let top_right = viewport.to_screen_space(rect.top_right(), ui.size()).floor();
               let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size()).floor();
               let bottom_left = viewport.to_screen_space(rect.bottom_left(), ui.size()).floor();
               let rect = Rect::new(top_left, bottom_right - top_left);
               let renderer = ui.render();
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
               self.draw_handle(renderer, top_left, Corner::TopLeft);
               self.draw_handle(renderer, top_right, Corner::TopRight);
               self.draw_handle(renderer, bottom_right, Corner::BottomRight);
               self.draw_handle(renderer, bottom_left, Corner::BottomLeft);
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

      if let Some(rect) = self.selection() {
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

fn format_vector(vector: Vector) -> String {
   format!("{:.0}, {:.0}", vector.x, vector.y)
}

fn label_width(font: &Font, text: &str) -> f32 {
   font.text_width(text).max(96.0)
}
