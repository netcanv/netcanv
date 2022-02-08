//! Tooltips that can be plugged into other controls, primarily buttons.

use std::borrow::Cow;

use netcanv_renderer::paws::{vector, AlignH, AlignV, Color, Layout, Rect, Vector};
use netcanv_renderer::Font as FontTrait;

use crate::backend::Font;
use crate::common::{SafeMath, VectorMath};

use super::{Input, Ui, UiInput};

/// The position of a tooltip relative to a UI group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TooltipPosition {
   Top,
   Left,
   Right,
}

/// Options for laying out a tooltip.
#[derive(Debug, Clone, Copy)]
pub struct TooltipLayout {
   /// Spacing from the group the tooltip is attached to.
   pub spacing: f32,
   /// The final position of the tooltip is clamped to the root group, padded with this amount of
   /// padding.
   pub root_padding: f32,
}

impl TooltipPosition {
   /// Computes the rectangle where a tooltip should be located.
   pub fn compute_rect(
      &self,
      ui: &Ui,
      group: Rect,
      size: Vector,
      TooltipLayout {
         spacing,
         root_padding,
      }: TooltipLayout,
   ) -> Rect {
      let Vector {
         x: width,
         y: height,
      } = size;
      let group_center = group.center();
      let center = match self {
         TooltipPosition::Top => group_center - vector(0.0, height / 2.0 + spacing),
         TooltipPosition::Left => group_center - vector(width / 2.0 + spacing, 0.0),
         TooltipPosition::Right => group_center + vector(width / 2.0 + spacing, 0.0),
      };
      let mut rect = Rect::new((center - size / 2.0).floor(), size);
      let root = ui.root_rect();
      rect.position.x =
         rect.position.x.safe_clamp(root_padding, root.width() - root_padding - rect.width());
      rect.position.y =
         rect.position.y.safe_clamp(root_padding, root.height() - root_padding - rect.height());
      rect
   }
}

/// Settings for drawing a tooltip.
#[derive(Clone)]
pub struct Tooltip<'s> {
   pub text: Cow<'s, str>,
   pub position: TooltipPosition,
}

impl<'s> Tooltip<'s> {
   pub fn new(text: impl Into<Cow<'s, str>>, position: TooltipPosition) -> Self {
      Self {
         text: text.into(),
         position,
      }
   }

   /// Shorthand for constructing a tooltip positioned above a group.
   pub fn top(text: impl Into<Cow<'s, str>>) -> Self {
      Self::new(text, TooltipPosition::Top)
   }

   /// Shorthand for constructing a tooltip positioned to the left of a group.
   pub fn left(text: impl Into<Cow<'s, str>>) -> Self {
      Self::new(text, TooltipPosition::Left)
   }

   /// Processes a tooltip. This should be called inside of the group that triggers the tooltip
   /// on hover.
   pub fn process(&self, ui: &mut Ui, input: &Input, font: &Font) {
      const PADDING: f32 = 16.0;
      const LAYOUT: TooltipLayout = TooltipLayout {
         spacing: PADDING * 1.5,
         root_padding: PADDING / 2.0,
      };

      if ui.has_mouse(input) {
         let width = font.text_width(&self.text) + PADDING;
         let height = font.height() + PADDING;
         let size = vector(width, height);
         let group = ui.rect();
         let rect = self.position.compute_rect(ui, group, size, LAYOUT);
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
