//! Color picker with palettes and multiple color spaces.

use netcanv_renderer::paws::{point, Color, Layout, Rect, Renderer};
use netcanv_renderer_opengl::winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::Image;
use crate::color::{AnyColor, Srgb};

use super::view::{Dimension, Dimensions, View};
use super::wm::{WindowContent, WindowContentArgs, WindowContentWrappers, WindowId, WindowManager};
use super::{Button, ButtonArgs, Input, Ui, UiInput};

/// Arguments for processing the color picker.
pub struct ColorPickerArgs<'a, 'wm> {
   pub assets: &'a Assets,
   pub wm: &'wm mut WindowManager,
   pub window_view: View,
}

/// Icons used by the color picker.
pub struct ColorPickerIcons {
   pub palette: Image,
}

/// A color picker.
pub struct ColorPicker {
   palette: [AnyColor; Self::NUM_COLORS],
   color: AnyColor,
   window_state: Option<PickerWindowState>,
}

impl ColorPicker {
   /// The number of colors in a palette.
   const NUM_COLORS: usize = 9;

   /// Creates a new color picker.
   pub fn new() -> Self {
      let palette = [
         0x100820, // black
         0xff003e, // red
         0xff7b00, // orange
         0xffff00, // yellow
         0x2dd70e, // green
         0x03cbfb, // aqua
         0x0868eb, // blue
         0xa315d7, // purple
         0xffffff, // white
      ]
      .map(|hex| Srgb::from_color(Color::rgb(hex)).into());
      Self {
         palette,
         color: palette[0],
         window_state: Some(PickerWindowState::Closed(())),
      }
   }

   /// Returns a view for the picker window. This view should be laid out and then passed back to
   /// `process` via [`ColorPickerArgs`].
   pub fn picker_window_view() -> View {
      View::new(PickerWindow::DIMENSIONS)
   }

   /// Returns the (paws) color that's currently selected.
   pub fn color(&self) -> Color {
      Srgb::from(self.color).to_color(1.0)
   }

   /// Processes the color palette.
   pub fn process(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      ColorPickerArgs {
         assets,
         wm,
         window_view,
      }: ColorPickerArgs,
   ) {
      // The palette.
      for &color in &self.palette {
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
            let color = Srgb::from(color).to_color(1.0);
            ui.render().fill(rect, color, 4.0);
         });
         ui.pop();
      }
      ui.space(16.0);

      // The color picker button.
      if Button::with_icon(
         ui,
         input,
         ButtonArgs {
            height: ui.height(),
            colors: &assets.colors.action_button,
            corner_radius: 0.0,
         },
         &assets.icons.color_picker.palette,
      )
      .clicked()
      {
         self.toggle_picker_window(wm, window_view)
      }
   }

   /// Toggles the picker window on or off, depending on whether it's already open or not.
   fn toggle_picker_window(&mut self, wm: &mut WindowManager, view: View) {
      match self.window_state.take().unwrap() {
         PickerWindowState::Open(window_id) => {
            let data = wm.close_window(window_id);
            self.window_state = Some(PickerWindowState::Closed(data));
         }
         PickerWindowState::Closed(data) => {
            let content = PickerWindow::new().background();
            let window_id = wm.open_window(view, content, data);
            self.window_state = Some(PickerWindowState::Open(window_id));
         }
      }
   }
}

enum PickerWindowState {
   Open(WindowId<PickerWindowData>),
   Closed(PickerWindowData),
}

type PickerWindowData = ();

struct PickerWindow {}

impl PickerWindow {
   /// The dimensions of the picker window.
   const DIMENSIONS: Dimensions = Dimensions {
      horizontal: Dimension::Constant(256.0),
      vertical: Dimension::Constant(256.0),
   };

   /// Creates the picker window's inner data.
   fn new() -> Self {
      Self {}
   }
}

impl WindowContent for PickerWindow {
   type Data = PickerWindowData;

   fn process(&mut self, args: WindowContentArgs, data: &mut Self::Data) {}
}
