use netcanv_renderer::paws::{point, vector, Color, Layout, Point, Rect, Renderer, Vector};
use netcanv_renderer::{Font as FontTrait, RenderBackend};
use winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::{Backend, Font, Image};
use crate::common::VectorMath;
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

pub struct Selection {
   icons: Icons,
   mouse_position: Point,
   selecting: bool,
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
         selecting: false,
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

   fn draw_handle(renderer: &mut Backend, position: Point) {
      renderer.fill_circle(position, Self::HANDLE_RADIUS + 2.0, Color::WHITE);
      renderer.fill_circle(position, Self::HANDLE_RADIUS, Self::COLOR);
   }
}

impl Tool for Selection {
   fn name(&self) -> &str {
      "Selection"
   }

   fn icon(&self) -> &Image {
      &self.icons.tool
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
      // Store the mouse position for the bottom bar display.
      self.mouse_position = mouse_position;

      // Check if the left mouse button was pressed, and if yes, start selecting.
      if input.mouse_button_just_pressed(MouseButton::Left) {
         self.selecting = true;
         // Here, anchor the selection to the mouse position.
         self.selection = Some(Rect::new(mouse_position, vector(0.0, 0.0)));
      }
      if input.mouse_button_just_released(MouseButton::Left) {
         self.selecting = false;
         // After the button is released and the selection's size is 0, deselect.
         if let Some(rect) = self.selection {
            if rect.width().abs() < 1.0 || rect.height().abs() < 1.0 {
               self.selection = None;
            }
         }
      }

      // While we're selecting, update the size of the rectangle such that its second corner
      // is at the mouse.
      if self.selecting {
         // The rectangle must be Some while we're selecting.
         let rect = self.selection.as_mut().unwrap();
         rect.size = mouse_position - rect.position;
      }
   }

   fn process_paint_canvas_overlays(&mut self, ToolArgs { ui, .. }: ToolArgs, viewport: &Viewport) {
      if let Some(rect) = self.selection() {
         ui.draw(|ui| {
            let top_left = viewport.to_screen_space(rect.top_left(), ui.size()).round();
            let top_right = viewport.to_screen_space(rect.top_right(), ui.size()).round();
            let bottom_right = viewport.to_screen_space(rect.bottom_right(), ui.size()).round();
            let bottom_left = viewport.to_screen_space(rect.bottom_left(), ui.size()).round();
            let rect = Rect::new(top_left, bottom_right - top_left);
            let renderer = ui.render();
            renderer.outline(rect, Self::COLOR, 0.0, 2.0);
            Self::draw_handle(renderer, top_left);
            Self::draw_handle(renderer, top_right);
            Self::draw_handle(renderer, bottom_right);
            Self::draw_handle(renderer, bottom_left);
         });
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
