//! Color picker with palettes and multiple color spaces.

use image::{Rgba, RgbaImage};
use netcanv_renderer::paws::{point, vector, Color, Layout, Padding, Rect, Renderer, Vector};
use netcanv_renderer::{Font, Framebuffer as FramebufferTrait, RenderBackend, ScalingFilter};
use netcanv_renderer_opengl::winit::event::MouseButton;
use strum::{EnumIter, EnumMessage};

use crate::assets::Assets;
use crate::backend::{Backend, Framebuffer, Image};
use crate::color::{AnyColor, Hsv, Srgb};
use crate::common::ColorMath;
use crate::ui::ValueSlider;

use super::view::{Dimension, Dimensions, View};
use super::wm::{WindowContent, WindowContentArgs, WindowContentWrappers, WindowId, WindowManager};
use super::{
   Button, ButtonArgs, ButtonState, Focus, Input, RadioButton, RadioButtonArgs, SliderStep,
   TextField, TextFieldArgs, TextFieldColors, Ui, UiInput, ValueSliderArgs,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, EnumMessage)]
enum ColorSpace {
   #[strum(message = "HSV")]
   Hsv,
   #[strum(message = "Oklab")]
   Oklab,
}

struct PickerWindowData {
   color: AnyColor,
   color_space: ColorSpace,
}

struct PickerWindow {
   /// The color space selector.
   color_space: RadioButton<ColorSpace>,

   /// The image of the color canvas - the large rectangular area that's used to pick
   /// a saturation and value (lightness).
   canvas_image: Framebuffer,
   /// The image of the color slider - the vertical slider used to pick hues.
   slider_image: Framebuffer,
   /// Whether the user is currently sliding the color values on the canvas.
   canvas_sliding: bool,
   /// Whether the user is currently sliding the hue value on the vertical slider.
   slider_sliding: bool,

   /// The text field containing the color's `#RRGGBB` hex code.
   hex_code: TextField,
   /// The channel and HSV sliders.
   sliders: [ValueSlider; 6],

   /// The previously selected color. If different from the previous frame, the widgets are
   /// updated to reflect the changes.
   previous_color: AnyColor,
}

impl PickerWindow {
   /// The dimensions of the picker window.
   const DIMENSIONS: Dimensions = Dimensions {
      horizontal: Dimension::Constant(448.0),
      vertical: Dimension::Constant(260.0),
   };

   /// The red channel adjustment slider.
   const R_SLIDER: usize = 0;
   /// The green channel adjustment slider.
   const G_SLIDER: usize = 1;
   /// The blue channel adjustment slider.
   const B_SLIDER: usize = 2;

   /// The hue adjustment slider.
   const H_SLIDER: usize = 3;
   /// The saturation adjustment slider.
   const S_SLIDER: usize = 4;
   /// The value adjustment slider.
   const V_SLIDER: usize = 5;

   /// Creates the picker window's inner data.
   fn new(renderer: &mut Backend, data: &PickerWindowData) -> Self {
      const CANVAS_RESOLUTION: u32 = 32;
      const SLIDER_RESOLUTION: (u32, u32) = (1, 64);
      let mut this = Self {
         color_space: RadioButton::new(data.color_space),

         canvas_image: renderer.create_framebuffer(CANVAS_RESOLUTION, CANVAS_RESOLUTION),
         slider_image: renderer.create_framebuffer(SLIDER_RESOLUTION.0, SLIDER_RESOLUTION.1),
         canvas_sliding: false,
         slider_sliding: false,

         hex_code: TextField::new(None),
         sliders: Self::rgb_sliders(Srgb::from(data.color).to_color(1.0)),

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
      this.update_widgets(data);
      this
   }

   /// Creates the picker window's outer data.
   fn new_data(default_color: AnyColor) -> PickerWindowData {
      PickerWindowData {
         color: default_color,
         color_space: ColorSpace::Hsv,
      }
   }

   /// Creates a set of RGB and HSV sliders for the given color.
   fn rgb_sliders(color: Color) -> [ValueSlider; 6] {
      let Color { r, g, b, .. } = color;
      let (r, g, b) = (r as f32, g as f32, b as f32);
      let Hsv { h, s, v } = Hsv::from(Srgb::from_color(color));
      let h = h * 60.0;
      [
         ValueSlider::new("R", r, 0.0, 255.0, SliderStep::Discrete(1.0)),
         ValueSlider::new("G", g, 0.0, 255.0, SliderStep::Discrete(1.0)),
         ValueSlider::new("B", b, 0.0, 255.0, SliderStep::Discrete(1.0)),
         ValueSlider::new("H", h, 0.0, 360.0, SliderStep::Discrete(1.0)),
         ValueSlider::new("S", s, 0.0, 1.0, SliderStep::Smooth),
         ValueSlider::new("V", v, 0.0, 1.0, SliderStep::Smooth),
      ]
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
         ColorSpace::Oklab => todo!(),
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
         ColorSpace::Oklab => todo!(),
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
               ColorSpace::Oklab => todo!(),
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
         data.color = match data.color_space {
            ColorSpace::Hsv => {
               let Hsv { s, v, .. } = Hsv::from(data.color);
               let h = y * 6.0;
               AnyColor::from(Hsv { h, s, v })
            }
            ColorSpace::Oklab => todo!(),
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
               ColorSpace::Oklab => todo!(),
            } * ui.width(),
         );
         let y = f32::round(
            match data.color_space {
               ColorSpace::Hsv => 1.0 - Hsv::from(data.color).v,
               ColorSpace::Oklab => todo!(),
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
            ColorSpace::Oklab => todo!(),
         };
      }

      ui.pop();
   }

   /// Processes the value display of the color picker.
   fn process_values(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      assets: &Assets,
      data: &mut PickerWindowData,
   ) {
      ui.push((ui.remaining_width(), ui.height()), Layout::Vertical);
      let color = Srgb::from(data.color).to_color(1.0);

      // The hex code text field.
      let text_color = if color.brightness() > 0.5 {
         Color::BLACK
      } else {
         Color::WHITE
      };
      let hex_code = self.hex_code.process(
         ui,
         input,
         TextFieldArgs {
            width: ui.width(),
            font: &assets.monospace.with_size(16.0),
            colors: &TextFieldColors {
               outline: Color::TRANSPARENT,
               outline_focus: Color::TRANSPARENT,
               fill: color,
               text: text_color.with_alpha(220),
               text_hint: text_color.with_alpha(127),
               ..assets.colors.text_field
            },
            hint: Some("RGB hex code"),
         },
      );
      if hex_code.done() || hex_code.unfocused() {
         if let Some(color) = Self::parse_hex_code(self.hex_code.text()) {
            let color = AnyColor::from(Srgb::from_color(color));
            data.color = color;
         }
         self.update_widgets(data);
      }
      ui.space(12.0);

      // The value sliders below the text field.
      let value_slider = ValueSliderArgs {
         color: assets.colors.slider,
         font: &assets.sans,
         label_width: Some(16.0),
      };
      let mut sliders_changed = [false; 6];
      for (i, slider) in self.sliders[Self::R_SLIDER..=Self::B_SLIDER].iter_mut().enumerate() {
         if slider.process(ui, input, value_slider).changed() {
            sliders_changed[i] = true;
         }
      }
      ui.space(8.0);
      for (i, slider) in self.sliders[Self::H_SLIDER..=Self::V_SLIDER].iter_mut().enumerate() {
         if slider.process(ui, input, value_slider).changed() {
            sliders_changed[i + Self::H_SLIDER] = true;
         }
      }

      macro_rules! update_color_channel {
         ($index:expr, $color_space:tt, $channel:tt, $max:expr) => {
            if sliders_changed[$index] {
               data.color = AnyColor::from($color_space {
                  $channel: self.sliders[$index].value() / $max,
                  ..$color_space::from(data.color)
               });
            }
         };
      }

      update_color_channel!(Self::R_SLIDER, Srgb, r, 255.0);
      update_color_channel!(Self::G_SLIDER, Srgb, g, 255.0);
      update_color_channel!(Self::B_SLIDER, Srgb, b, 255.0);
      update_color_channel!(Self::H_SLIDER, Hsv, h, 60.0);
      update_color_channel!(Self::S_SLIDER, Hsv, s, 1.0);
      update_color_channel!(Self::V_SLIDER, Hsv, v, 1.0);

      if sliders_changed.iter().any(|&changed| changed) {
         self.update_widgets(data);
      }

      ui.pop();
   }

   /// Parses a hex code into a color. If the given text is not a valid hex code, returns `None`.
   fn parse_hex_code(text: &str) -> Option<Color> {
      // Empty string? Not a hex code.
      if text.len() == 0 {
         return None;
      }
      // Strip the optional, leading #.
      let text = text.strip_prefix('#').unwrap_or(text);
      match text.len() {
         3 => {
            // With #RGB colors, we do some byte manipulation to repeat the R, G, B quartets
            // such that we end up with an #RRGGBB color.
            let hex = u32::from_str_radix(text, 16).ok()?;
            let (r, g, b) = (hex & 0xF, (hex >> 4) & 0xF, (hex >> 8) & 0xF);
            let (r, g, b) = (r | (r << 4), g | (g << 4), b | (b << 4));
            let hex = r | (g << 8) | (b << 16);
            Some(Color::rgb(hex))
         }
         6 => {
            // With #RRGGBB colors no manipulation needs to be done, so we just parse and
            // interpret the color literally.
            let hex = u32::from_str_radix(text, 16).ok()?;
            Some(Color::rgb(hex))
         }
         _ => None,
      }
   }

   /// Updates the widgets to reflect the currently picked color.
   fn update_widgets(&mut self, data: &PickerWindowData) {
      let color = Srgb::from(data.color).to_color(1.0);

      // Make sure the color canvas shows the correct hue.
      Self::update_canvas(
         &mut self.canvas_image,
         Hsv::from(data.color).h,
         data.color_space,
      );

      // Update the hex code in the text field.
      if !self.hex_code.focused() {
         self.hex_code.set_text(format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b));
      }

      // Update the sliders.
      self.sliders[Self::R_SLIDER].set_value(Srgb::from(data.color).r * 255.0);
      self.sliders[Self::G_SLIDER].set_value(Srgb::from(data.color).g * 255.0);
      self.sliders[Self::B_SLIDER].set_value(Srgb::from(data.color).b * 255.0);
      self.sliders[Self::H_SLIDER].set_value(Hsv::from(data.color).h * 60.0);
      self.sliders[Self::S_SLIDER].set_value(Hsv::from(data.color).s);
      self.sliders[Self::V_SLIDER].set_value(Hsv::from(data.color).v);
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
      ui.push(ui.size(), Layout::Vertical);

      // The title bar and color space selector.
      ui.push((ui.width(), 40.0), Layout::Horizontal);
      ui.pad((12.0, 0.0));
      ui.offset(vector(0.0, 8.0));
      self.color_space.with_text(
         ui,
         input,
         RadioButtonArgs {
            height: 24.0,
            colors: &assets.colors.radio_button,
            corner_radius: 11.5,
         },
         &assets.sans,
      );
      ui.pop();

      // Process the group encompassing the color canvas and slider.
      ui.push(ui.remaining_size(), Layout::Horizontal);
      ui.pad(Padding {
         top: 0.0,
         ..Padding::even(12.0)
      });

      self.process_canvas(ui, input, data);
      ui.space(12.0);
      self.process_slider(ui, input, data);
      ui.space(12.0);
      self.process_values(ui, input, assets, data);

      ui.pop();

      ui.pop();

      if data.color != self.previous_color {
         self.update_widgets(data);
      }
      self.previous_color = data.color;
   }
}
