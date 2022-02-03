//! Tooltips that can be plugged into other controls, primarily buttons.

use std::borrow::Cow;

use netcanv_renderer::paws::{vector, AlignH, AlignV, Color, Layout, Rect};
use netcanv_renderer::Font as FontTrait;

use crate::backend::Font;
use crate::common::VectorMath;

use super::{Input, Ui, UiInput};

/// The position of a tooltip relative to a UI group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TooltipPosition {
   Top,
   Left,
   Right,
}

/// Settings for drawing a tooltip.
#[derive(Clone)]
pub struct Tooltip {
   pub text: Cow<'static, str>,
   pub position: TooltipPosition,
}

impl Tooltip {
   pub fn new(text: impl Into<Cow<'static, str>>, position: TooltipPosition) -> Self {
      Self {
         text: text.into(),
         position,
      }
   }

   /// Shorthand for constructing a tooltip positioned above a group.
   pub fn top(text: impl Into<Cow<'static, str>>) -> Self {
      Self::new(text, TooltipPosition::Top)
   }

   /// Processes a tooltip. This should be called inside of the group that triggers the tooltip
   /// on hover.
   pub fn process(&self, ui: &mut Ui, input: &Input, font: &Font) {
      const PADDING: f32 = 16.0;
      const SPACING: f32 = PADDING * 1.5;
      const SCREEN_PADDING: f32 = PADDING / 2.0;

      if ui.has_mouse(input) {
         let width = font.text_width(&self.text) + PADDING;
         let height = font.height() + PADDING;
         let size = vector(width, height);
         let group = ui.rect();
         let group_center = group.center();
         let center = match self.position {
            TooltipPosition::Top => group_center - vector(0.0, height / 2.0 + SPACING),
            TooltipPosition::Left => group_center - vector(width / 2.0 + SPACING, 0.0),
            TooltipPosition::Right => group_center + vector(width / 2.0 + SPACING, 0.0),
         };
         let mut rect = Rect::new((center - size / 2.0).floor(), size);
         let root = ui.root_rect();
         rect.position.x =
            rect.position.x.clamp(SCREEN_PADDING, root.width() - SCREEN_PADDING - rect.width());
         rect.position.y = rect.position.y.clamp(
            SCREEN_PADDING,
            root.height() - SCREEN_PADDING - rect.height(),
         );
         ui.push((0.0, 0.0), Layout::Freeform);
         ui.set_position(rect.position);
         ui.push(rect.size, Layout::Freeform);
         ui.fill_rounded(Color::BLACK.with_alpha(192), 4.0);
         ui.text(
            font,
            &self.text,
            Color::WHITE,
            (AlignH::Center, AlignV::Middle),
         );
         ui.pop();
         ui.pop();
      }
   }
}
