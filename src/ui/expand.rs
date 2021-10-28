//! Expand widgets group elements together and form what's called an "accordion".

use netcanv_renderer::Font as FontTrait;
use paws::{AlignH, AlignV, Color, Layout, SizedImage};

use crate::{
   backend::{Font, Image},
   ui::*,
};

/// An Expand's state.
pub struct Expand {
   expanded: bool,
}

/// The icons to use for the expanded and shrinked state.
pub struct ExpandIcons {
   pub expand: Image,
   pub shrink: Image,
}

/// The color scheme of an Expand.
#[derive(Clone)]
pub struct ExpandColors {
   pub text: Color,
   pub icon: Color,
   pub hover: Color,
   pub pressed: Color,
}

/// Processing arguments for an Expand.
#[derive(Clone, Copy)]
pub struct ExpandArgs<'a, 'b, 'c, 'd> {
   pub label: &'a str,
   pub icons: &'b ExpandIcons,
   pub colors: &'c ExpandColors,
   pub font: &'d Font,
}

/// The result result of processing an `Expand`.
pub struct ExpandProcessResult {
   expanded: bool,
   just_clicked: bool,
}

impl Expand {
   /// Creates a new Expand.
   pub fn new(expanded: bool) -> Self {
      Self { expanded }
   }

   /// Processes an Expand.
   #[must_use]
   pub fn process(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      ExpandArgs {
         label,
         font,
         icons,
         colors,
      }: ExpandArgs,
   ) -> ExpandProcessResult {
      let mut result = ExpandProcessResult {
         expanded: false,
         just_clicked: false,
      };
      let icon = if self.expanded {
         &icons.shrink
      } else {
         &icons.expand
      };
      let height = icon.height() as f32;

      ui.push((ui.width(), height), Layout::Freeform);

      // icon and label
      ui.push(ui.size(), Layout::Horizontal);
      // ui.icon(icon, colors.icon, Some((height, height)));
      ui.space(8.0);
      ui.push((ui.remaining_width(), ui.height()), Layout::Freeform);
      // ui.set_font_size(font_size);
      ui.text(font, label, colors.text, (AlignH::Left, AlignV::Middle));
      let width = height + 8.0 + font.text_width(label);
      ui.pop();
      ui.pop();

      // visible area
      ui.push((width, ui.height()), Layout::Freeform);
      if ui.has_mouse(input) {
         let pressed = input.mouse_button_is_down(MouseButton::Left);
         // underline
         // TODO(renderer): expand underline
         // ui.draw_on_canvas(canvas, |canvas| {
         //    let underline_color: Color = if pressed {
         //       colors.pressed
         //    } else {
         //       colors.hover
         //    }
         //    .into();
         //    let y = height * 1.1;
         //    let mut paint = Paint::new(underline_color, None);
         //    paint.set_anti_alias(false);
         //    paint.set_style(paint::Style::Stroke);
         //    canvas.draw_line((0.0, y), (width, y), &paint);
         // });
         // events
         if input.mouse_button_just_released(MouseButton::Left) {
            self.expanded = !self.expanded;
            result.just_clicked = true;
         }
      }
      ui.pop();

      ui.pop();

      result.expanded = self.expanded;
      result
   }
}

impl ExpandProcessResult {
   /// Shrinks the other Expand if the Expand this `ExpandProcessResult` is a result of was just
   /// expanded.
   pub fn mutually_exclude(self, other: &mut Expand) -> Self {
      if self.expanded && self.just_clicked {
         other.expanded = false;
      }
      self
   }

   /// Returns whether the Expand is expanded.
   pub fn expanded(self) -> bool {
      self.expanded
   }
}
