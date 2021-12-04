//! Color picker with palettes and multiple color spaces.

use image::{Rgba, RgbaImage};
use netcanv_renderer::paws::{point, vector, Color, Layout, Rect, Renderer, Vector};
use netcanv_renderer::{Framebuffer as FramebufferTrait, RenderBackend, ScalingFilter};
use netcanv_renderer_opengl::winit::event::MouseButton;

use crate::assets::Assets;
use crate::backend::{Backend, Framebuffer, Image};
use crate::color::{AnyColor, Hsv, Srgb};

use super::view::{Dimension, Dimensions, View};
use super::wm::{WindowContent, WindowContentArgs, WindowContentWrappers, WindowId, WindowManager};
use super::{Button, ButtonArgs, ButtonState, Input, Ui, UiInput};

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
         window_state: Some(PickerWindowState::Closed(PickerWindow::new_data(
            palette[0],
         ))),
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
      for color in self.palette {
         ui.push((16.0, ui.height()), Layout::Freeform);
         let y_offset = ui.height()
            * if self.color == color {
               0.5
            } else if ui.hover(&input) {
               0.7
            } else {
               0.8
            };
         let y_offset = y_offset.round();
         if ui.hover(&input) && input.mouse_button_just_pressed(MouseButton::Left) {
            self.window_data_mut(wm).color = color;
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
         self.toggle_picker_window(ui, wm, window_view)
      }

      // The color variable, cached from what was chosen in the picker window.
      self.color = self.window_data(wm).color;
   }

   /// Toggles the picker window on or off, depending on whether it's already open or not.
   fn toggle_picker_window(&mut self, renderer: &mut Backend, wm: &mut WindowManager, view: View) {
      match self.window_state.take().unwrap() {
         PickerWindowState::Open(window_id) => {
            let data = wm.close_window(window_id);
            self.window_state = Some(PickerWindowState::Closed(data));
         }
         PickerWindowState::Closed(data) => {
            let content = PickerWindow::new(renderer, &data).background();
            let window_id = wm.open_window(view, content, data);
            self.window_state = Some(PickerWindowState::Open(window_id));
         }
      }
   }

   /// Returns the picker window's data, no matter if it's open.
   fn window_data<'d>(&'d self, wm: &'d WindowManager) -> &'d PickerWindowData {
      let state = self.window_state.as_ref().unwrap();
      match state {
         PickerWindowState::Open(window_id) => wm.window_data(window_id),
         PickerWindowState::Closed(data) => data,
      }
   }

   /// Same as [`Self::window_data`], but returns a mutable reference.
   fn window_data_mut<'d>(&'d mut self, wm: &'d mut WindowManager) -> &'d mut PickerWindowData {
      let state = self.window_state.as_mut().unwrap();
      match state {
         PickerWindowState::Open(window_id) => wm.window_data_mut(window_id),
         PickerWindowState::Closed(data) => data,
      }
   }
}

enum PickerWindowState {
   Open(WindowId<PickerWindowData>),
   Closed(PickerWindowData),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorSpace {
   Hsv,
}

struct PickerWindowData {
   color: AnyColor,
   color_space: ColorSpace,
}

struct PickerWindow {
   /// The image of the color canvas - the large rectangular area that's used to pick
   /// a saturation and value (lightness).
   canvas_image: Framebuffer,
   /// The image of the color slider - the vertical slider used to pick hues.
   slider_image: Framebuffer,

   canvas_sliding: bool,
   slider_sliding: bool,
   previous_color: AnyColor,
}

impl PickerWindow {
   /// The dimensions of the picker window.
   const DIMENSIONS: Dimensions = Dimensions {
      horizontal: Dimension::Constant(512.0),
      vertical: Dimension::Constant(256.0),
   };

   /// Creates the picker window's inner data.
   fn new(renderer: &mut Backend, data: &PickerWindowData) -> Self {
      const CANVAS_RESOLUTION: u32 = 32;
      const SLIDER_RESOLUTION: (u32, u32) = (1, 64);
      let mut this = Self {
         canvas_image: renderer.create_framebuffer(CANVAS_RESOLUTION, CANVAS_RESOLUTION),
         slider_image: renderer.create_framebuffer(SLIDER_RESOLUTION.0, SLIDER_RESOLUTION.1),
         canvas_sliding: false,
         slider_sliding: false,
         previous_color: data.color,
      };
      this.slider_image.set_scaling_filter(ScalingFilter::Linear);
      this.canvas_image.set_scaling_filter(ScalingFilter::Linear);
      Self::update_slider(&mut this.slider_image, data.color_space);
      Self::update_canvas(
         &mut this.canvas_image,
         Hsv::from(data.color).h,
         data.color_space,
      );
      this
   }

   /// Creates the picker window's outer data.
   fn new_data(default_color: AnyColor) -> PickerWindowData {
      PickerWindowData {
         color: default_color,
         color_space: ColorSpace::Hsv,
      }
   }

   /// Renders the slider for the given color space, to the given framebuffer.
   fn update_slider(framebuffer: &mut Framebuffer, color_space: ColorSpace) {
      let (width, height) = framebuffer.size();
      let image = match color_space {
         ColorSpace::Hsv => RgbaImage::from_fn(width, height, |_x, y| {
            let hue = y as f32 / height as f32 * 6.0;
            let color = Srgb::from(Hsv {
               h: hue,
               s: 1.0,
               v: 1.0,
            })
            .to_color(1.0);
            Rgba([color.r, color.g, color.b, color.a])
         }),
      };
      framebuffer.upload_rgba((0, 0), (width, height), &image);
   }

   /// Renders the canvas for the given hue and color space, to the given framebuffer.
   fn update_canvas(framebuffer: &mut Framebuffer, hue: f32, color_space: ColorSpace) {
      let (width, height) = framebuffer.size();
      let image = match color_space {
         ColorSpace::Hsv => RgbaImage::from_fn(width, height, |x, y| {
            let saturation = x as f32 / (width - 1) as f32;
            let value = 1.0 - y as f32 / (height - 1) as f32;
            let color = Srgb::from(Hsv {
               h: hue,
               s: saturation,
               v: value,
            })
            .to_color(1.0);
            Rgba([color.r, color.g, color.b, color.a])
         }),
      };
      framebuffer.upload_rgba((0, 0), (width, height), &image);
   }

   /// Processes the hue slider.
   fn process_slider(&mut self, ui: &mut Ui, input: &Input, data: &mut PickerWindowData) {
      ui.push((24.0, ui.height()), Layout::Freeform);
      let rect = ui.rect();
      ui.render().framebuffer(rect, &self.slider_image);

      ui.draw(|ui| {
         let y = f32::round(
            match data.color_space {
               ColorSpace::Hsv => Hsv::from(data.color).h / 6.0,
            } * ui.height(),
         );
         let width = ui.width();
         let indicator_radius = 4.0;
         ui.render().outline(
            Rect::new(
               point(-2.0, y - indicator_radius - 1.0),
               vector(width + 4.0, indicator_radius * 2.0 + 2.0),
            ),
            Color::BLACK,
            2.0,
            1.0,
         );
         ui.render().outline(
            Rect::new(
               point(-1.0, y - indicator_radius),
               vector(width + 2.0, indicator_radius * 2.0),
            ),
            Color::WHITE,
            2.0,
            1.0,
         );
      });

      match input.action(MouseButton::Left) {
         (true, ButtonState::Pressed) if ui.hover(input) => self.slider_sliding = true,
         (_, ButtonState::Released) => self.slider_sliding = false,
         _ => (),
      }

      if self.slider_sliding {
         let y = ui.mouse_position(input).y / ui.height();
         let y = y.clamp(0.0, 1.0 - f32::EPSILON);
         let previous_color = data.color;
         data.color = match data.color_space {
            ColorSpace::Hsv => {
               let Hsv { s, v, .. } = Hsv::from(data.color);
               let h = y * 6.0;
               AnyColor::from(Hsv { h, s, v })
            }
         };
      }

      ui.pop();
   }

   /// Processes the color canvas.
   fn process_canvas(&mut self, ui: &mut Ui, input: &Input, data: &mut PickerWindowData) {
      ui.push((ui.height(), ui.height()), Layout::Freeform);
      let rect = ui.rect();
      ui.render().framebuffer(rect, &self.canvas_image);

      ui.draw(|ui| {
         let x = f32::round(
            match data.color_space {
               ColorSpace::Hsv => Hsv::from(data.color).s,
            } * ui.width(),
         );
         let y = f32::round(
            match data.color_space {
               ColorSpace::Hsv => 1.0 - Hsv::from(data.color).v,
            } * ui.height(),
         );
         let radius = 4.0;
         ui.render().outline_circle(point(x, y), radius + 1.0, Color::BLACK, 1.0);
         ui.render().outline_circle(point(x, y), radius, Color::WHITE, 1.0);
      });

      match input.action(MouseButton::Left) {
         (true, ButtonState::Pressed) if ui.hover(input) => self.canvas_sliding = true,
         (_, ButtonState::Released) => self.canvas_sliding = false,
         _ => (),
      }

      if self.canvas_sliding {
         let Vector { x, y } = ui.mouse_position(input) / ui.size();
         let (x, y) = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
         data.color = match data.color_space {
            ColorSpace::Hsv => {
               let (s, v) = (x, 1.0 - y);
               let h = Hsv::from(data.color).h;
               AnyColor::from(Hsv { h, s, v })
            }
         };
      }

      ui.pop();
   }
}

impl WindowContent for PickerWindow {
   type Data = PickerWindowData;

   fn process(
      &mut self,
      WindowContentArgs {
         ui, input, assets, ..
      }: WindowContentArgs,
      data: &mut Self::Data,
   ) {
      ui.pad(12.0);

      // The group encompassing the color canvas and slider.
      ui.push(ui.size(), Layout::Horizontal);

      // The color canvas.
      self.process_canvas(ui, input, data);
      ui.space(12.0);

      // The color slider.
      self.process_slider(ui, input, data);

      if data.color != self.previous_color {
         Self::update_canvas(
            &mut self.canvas_image,
            Hsv::from(data.color).h,
            data.color_space,
         );
         self.previous_color = data.color;
      }

      ui.pop();
      self.previous_color = data.color;
   }
}
