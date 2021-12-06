//! Window content wrappers for the window manager.

use netcanv_renderer::paws::{AlignH, AlignV, Color, Layout, Padding};
use netcanv_renderer::{Image as ImageTrait, RenderBackend};
use netcanv_renderer_opengl::winit::event::MouseButton;

use crate::backend::Image;
use crate::ui::{Input, Ui, UiInput};

use super::{HitTest, WindowContent, WindowContentArgs};

/// Draws a gray, panel background below a window.
///
/// Create using [`WindowContent::background`].
pub struct Background<C, D>
where
   C: WindowContent<Data = D>,
{
   inner: C,
}

impl<C, D> WindowContent for Background<C, D>
where
   C: WindowContent<Data = D>,
{
   type Data = D;

   fn process(&mut self, args: &mut WindowContentArgs, data: &mut Self::Data) {
      let WindowContentArgs { ui, assets, .. } = args;
      ui.fill_rounded(assets.colors.panel, 4.0);
      self.inner.process(args, data);
   }
}

#[derive(Clone)]
pub struct WindowButtonColors {
   pub normal_fill: Color,
   pub normal_icon: Color,
   pub hover_fill: Color,
   pub hover_icon: Color,
   pub pressed_fill: Color,
   pub pressed_icon: Color,
}

/// The colors used by window buttons.
#[derive(Clone)]
pub struct WindowButtonsColors {
   pub close: WindowButtonColors,
   pub pin: WindowButtonColors,
   pub pinned: WindowButtonColors,
}

/// The arguments for laying out window buttons.
pub struct WindowButtonStyle {
   /// Padding applied from the top-right corner of the screen.
   pub padding: Padding,
}

/// Draws the close and pin buttons on top of the window.
pub struct Buttons<C, D>
where
   C: WindowContent<Data = D>,
{
   inner: C,
   style: WindowButtonStyle,
}

impl<C, D> Buttons<C, D>
where
   C: WindowContent<Data = D>,
{
   /// Processes the button, drawing it with the given color scheme and icon.
   ///
   /// Returns whether the button is currently being hovered
   fn process_button(
      &self,
      ui: &mut Ui,
      input: &Input,
      colors: &WindowButtonColors,
      icon: &Image,
   ) -> bool {
      ui.push((24.0, 24.0), Layout::Freeform);

      ui.fill_rounded(
         if ui.hover(input) {
            if input.mouse_button_is_down(MouseButton::Left) {
               colors.pressed_fill
            } else {
               colors.hover_fill
            }
         } else {
            colors.normal_fill
         },
         12.0,
      );

      let close_rect = ui.rect();
      let icon_color = if ui.hover(input) {
         if input.mouse_button_is_down(MouseButton::Left) {
            colors.pressed_icon
         } else {
            colors.hover_icon
         }
      } else {
         colors.normal_icon
      };
      ui.image(close_rect, &icon.colorized(icon_color));

      let hover = ui.hover(input);
      ui.pop();

      hover
   }
}

impl<C, D> WindowContent for Buttons<C, D>
where
   C: WindowContent<Data = D>,
{
   type Data = D;

   fn process(&mut self, args: &mut WindowContentArgs, data: &mut Self::Data) {
      self.inner.process(args, data);

      let WindowContentArgs {
         ui,
         input,
         assets,
         pinned,
         ..
      } = args;
      let &mut pinned = pinned;
      ui.push(ui.size(), Layout::Freeform);
      ui.pad(self.style.padding);

      ui.push((52.0, 24.0), Layout::Horizontal);
      ui.align((AlignH::Right, AlignV::Top));

      if self.process_button(
         ui,
         input,
         if pinned {
            &assets.colors.window_buttons.pinned
         } else {
            &assets.colors.window_buttons.pin
         },
         if pinned {
            &assets.icons.window.pinned
         } else {
            &assets.icons.window.pin
         },
      ) {
         *args.hit_test = HitTest::PinButton;
      }

      ui.space(4.0);

      if self.process_button(
         ui,
         input,
         &assets.colors.window_buttons.close,
         &assets.icons.window.close,
      ) {
         *args.hit_test = HitTest::CloseButton;
      }

      ui.pop();

      ui.pop();
   }
}

pub trait WindowContentWrappers<C, D>
where
   C: WindowContent<Data = D>,
{
   /// Creates a window content wrapper that draws a gray background below the content.
   fn background(self) -> Background<C, D>;

   /// Creates a window content wrapper that draws the pin and close buttons on top of the content.
   fn buttons(self, style: WindowButtonStyle) -> Buttons<C, D>;
}

impl<C, D> WindowContentWrappers<C, D> for C
where
   C: WindowContent<Data = D>,
{
   fn background(self) -> Background<C, D> {
      Background { inner: self }
   }

   fn buttons(self, style: WindowButtonStyle) -> Buttons<C, D> {
      Buttons { inner: self, style }
   }
}
