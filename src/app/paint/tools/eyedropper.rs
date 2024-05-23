use crate::backend::winit::event::MouseButton;
use netcanv_renderer::paws::{AlignH, AlignV, Color, Layout, Point};

use crate::assets::Assets;
use crate::backend::{Backend, Image};
use crate::common::ColorMath;
use crate::config::config;
use crate::keymap::KeyBinding;
use crate::paint_canvas::PaintCanvas;
use crate::ui::{view, ColorPicker, ColorPickerArgs};
use crate::viewport::Viewport;

use super::{Tool, ToolArgs};

pub struct EyedropperTool {
   icon: Image,
   color: Color,
}

impl EyedropperTool {
   /// Creates an instance of the eyedropper tool.
   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icon: Assets::load_svg(
            renderer,
            include_bytes!("../../../assets/icons/eyedropper.svg"),
         ),
         color: Color::BLACK,
      }
   }
}

impl Tool for EyedropperTool {
   fn name(&self) -> &'static str {
      "eyedropper"
   }

   fn icon(&self) -> &Image {
      &self.icon
   }

   fn key_shortcut(&self) -> KeyBinding {
      config().keymap.tools.eyedropper.clone()
   }

   fn process_paint_canvas_input(
      &mut self,
      ToolArgs {
         ui,
         input,
         global_controls,
         ..
      }: ToolArgs,
      paint_canvas: &mut PaintCanvas,
      viewport: &Viewport,
   ) {
      if input.mouse_active() {
         let Point { x, y } = viewport.to_viewport_space(input.mouse_position(), ui.size());
         self.color = paint_canvas.get_pixel(ui, (x as i64, y as i64));

         if input.mouse_button_is_down(MouseButton::Left) {
            if self.color.a == 0 {
               global_controls.color_picker.set_eraser(true);
            } else {
               global_controls.color_picker.set_color(self.color);
            }
         }
      }
   }

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
            show_eraser: true,
         },
      );

      if self.color.a != 0 {
         ui.space(16.0);
         ui.push((72.0, ui.height()), Layout::Freeform);
         ui.fill(self.color);
         ui.text(
            &assets.monospace,
            &format!(
               "#{:02x}{:02x}{:02x}",
               self.color.r, self.color.g, self.color.b
            ),
            if self.color.brightness() > 0.5 {
               Color::BLACK
            } else {
               Color::WHITE
            }
            .with_alpha(220),
            (AlignH::Center, AlignV::Middle),
         );
         ui.pop();
      }
   }
}
