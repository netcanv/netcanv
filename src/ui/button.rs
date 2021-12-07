//! Pressable buttons.

use netcanv_renderer::paws::{AlignH, AlignV, Color, Layout};
use netcanv_renderer::Font as FontTrait;

use crate::backend::{Font, Image};
use crate::ui::*;

use super::input::Input;

/// A button. This simply acts as a namespace for button-related functionality.
pub struct Button;

/// The color scheme of a button.
#[derive(Clone)]
pub struct ButtonColors {
   pub fill: Color,
   pub outline: Color,
   pub text: Color,
   pub hover: Color,
   pub pressed: Color,
}

impl ButtonColors {
   /// Selects button colors for a togglable button.
   pub fn toggle<'c>(cond: bool, off: &'c ButtonColors, on: &'c ButtonColors) -> &'c ButtonColors {
      if cond {
         on
      } else {
         off
      }
   }
}

/// The layout and color scheme arguments for processing the button.
#[derive(Clone, Copy)]
pub struct ButtonArgs<'c> {
   pub height: f32,
   pub colors: &'c ButtonColors,
   pub corner_radius: f32,
}

/// The result of button interaction computed after processing it.
pub struct ButtonProcessResult {
   clicked: bool,
}

impl Button {
   /// Processes a generic button.
   ///
   /// The `width_hint` can be provided to specify how wide the button is ahead of time. This must
   /// be provided for the button to work with reversed layouts.
   ///
   /// `extra` is used for rendering extra things on top of the button.
   pub fn process(
      ui: &mut Ui,
      input: &Input,
      ButtonArgs {
         height,
         colors,
         corner_radius,
      }: ButtonArgs,
      width_hint: Option<f32>,
      extra: impl FnOnce(&mut Ui),
   ) -> ButtonProcessResult {
      // horizontal because we need to fit() later
      ui.push((width_hint.unwrap_or(0.0), height), Layout::Horizontal);
      ui.fill_rounded(colors.fill, corner_radius);

      extra(ui);
      ui.fit();

      ui.outline_rounded(colors.outline, corner_radius, 1.0);
      if ui.hover(input) {
         let fill_color = match input.action(MouseButton::Left) {
            (true, ButtonState::Pressed | ButtonState::Down) => colors.pressed,
            _ => colors.hover,
         };
         ui.fill_rounded(fill_color, corner_radius);
      }
      let clicked = ui.clicked(input, MouseButton::Left);

      ui.pop();

      ButtonProcessResult { clicked }
   }

   /// Processes a button with text rendered on top.
   pub fn with_text(
      ui: &mut Ui,
      input: &Input,
      args: ButtonArgs,
      font: &Font,
      text: &str,
   ) -> ButtonProcessResult {
      let width = font.text_width(text) + args.height;
      Self::process(ui, input, args, Some(width), |ui| {
         ui.push((width, ui.height()), Layout::Freeform);
         ui.text(
            font,
            text,
            args.colors.text,
            (AlignH::Center, AlignV::Middle),
         );
         ui.pop();
      })
   }

   /// Processes a button with a square icon rendered on top.
   pub fn with_icon(
      ui: &mut Ui,
      input: &Input,
      args: ButtonArgs,
      icon: &Image,
   ) -> ButtonProcessResult {
      Self::process(ui, input, args, Some(args.height), |ui| {
         ui.icon(
            icon,
            args.colors.text,
            Some(vector(args.height, args.height)),
         );
      })
   }
}

impl ButtonProcessResult {
   pub fn clicked(self) -> bool {
      self.clicked
   }
}
