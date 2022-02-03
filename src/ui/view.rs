//! Views and view layouting.

use netcanv_renderer::paws::{point, vector, Layout, Point, Rect, Vector};

use crate::token::Token;

use super::{Input, Ui, UiInput};

/// A dimension. Unlike concrete sizes, dimensions can be specified relative to the
/// parent container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
   /// A constant size.
   Constant(f32),
   /// A percentage of the parent view's size.
   Percentage(f32),
   /// A percentage of the remaining space in the parent view.
   Rest(f32),
}

impl Dimension {
   /// Computes the given dimension, according to the given cursor position, and parent size.
   fn compute(&self, cursor: f32, parent_size: f32) -> f32 {
      match self {
         &Dimension::Constant(value) => value,
         Dimension::Percentage(factor) => parent_size * factor,
         Dimension::Rest(factor) => (parent_size - cursor) * factor,
      }
   }
}

impl From<f32> for Dimension {
   /// Creates a constant dimension.
   fn from(value: f32) -> Self {
      Self::Constant(value)
   }
}

/// Horizontal and vertical dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dimensions {
   pub horizontal: Dimension,
   pub vertical: Dimension,
}

impl Dimensions {
   pub fn new(horizontal: impl Into<Dimension>, vertical: impl Into<Dimension>) -> Self {
      Self {
         horizontal: horizontal.into(),
         vertical: vertical.into(),
      }
   }
}

impl<T, U> From<(T, U)> for Dimensions
where
   T: Into<Dimension>,
   U: Into<Dimension>,
{
   fn from(dimensions: (T, U)) -> Self {
      Self::new(dimensions.0, dimensions.1)
   }
}

/// A view.
#[derive(Debug, Clone)]
pub struct View {
   /// The ID of the view. This is used for specifying mouse areas in the input layer.
   id: usize,
   /// The position of the view.
   pub position: Point,
   /// The dimensions of the view.
   pub dimensions: Dimensions,
   /// The computed size of the view.
   size: Option<Vector>,
}

static NO_MOUSE_AREA: usize = 0;
static MOUSE_AREA: Token = Token::new(NO_MOUSE_AREA + 1);

impl View {
   /// Creates a new view with the provided dimensions.
   ///
   /// The initial position of the view is `(0.0, 0.0)`.
   pub fn new(dimensions: impl Into<Dimensions>) -> Self {
      Self {
         id: MOUSE_AREA.next(),
         position: point(0.0, 0.0),
         dimensions: dimensions.into(),
         size: None,
      }
   }

   /// Returns the computed size of the view.
   ///
   /// Panics if the size has not been computed yet.
   pub fn size(&self) -> Vector {
      self.size.expect("attempt to get computed size of view that has not been laid out yet")
   }

   /// Returns the width of the view.
   ///
   /// Panics if the size has not been computed yet.
   pub fn width(&self) -> f32 {
      self.size().x
   }

   /// Returns the height of the view.
   ///
   /// Panics if the size has not been computed yet.
   pub fn height(&self) -> f32 {
      self.size().y
   }

   /// Returns the view's rectangle.
   ///
   /// Panics if the size has not been computed yet.
   pub fn rect(&self) -> Rect {
      Rect {
         position: self.position,
         size: self.size(),
      }
   }

   /// Creates a new view, whose size is the current group in the given UI.
   ///
   /// The initial position of the view is `(0.0, 0.0)`.
   pub fn group_sized(ui: &Ui) -> Self {
      let Vector {
         x: width,
         y: height,
      } = ui.size();
      Self::new((Dimension::Constant(width), Dimension::Constant(height)))
   }

   /// Begins rendering inside the view.
   pub fn begin(&self, ui: &mut Ui, input: &mut Input, layout: Layout) {
      // Push an intermediary, zero-sized group onto the stack, such that the current group's
      // layout does not get displaced.
      ui.push((0.0, 0.0), Layout::Freeform);
      ui.push(self.size(), layout);
      ui.set_position(self.position);
      input.set_mouse_area(self.id, ui.has_mouse(input));
   }

   /// Ends rendering inside the view.
   pub fn end(&self, ui: &mut Ui) {
      ui.pop();
      ui.pop();
   }

   /// Returns whether the view contains the mouse cursor.
   pub fn has_mouse(&self, input: &Input) -> bool {
      let mouse_position = input.mouse_position();
      mouse_position.x >= self.position.x
         && mouse_position.x < self.position.x + self.size().x
         && mouse_position.y >= self.position.y
         && mouse_position.y < self.position.y + self.size().y
   }
}

/// Functions for laying out views.
pub mod layout {
   use netcanv_renderer::paws::{AlignH, AlignV, Alignment, Padding};

   use super::*;

   /// Creates a new view with an amount of padding applied.
   ///
   /// The given view must have a computed size.
   pub fn padded(view: &View, padding: impl Into<Padding>) -> View {
      let padding = padding.into();
      let position = view.position + vector(padding.left, padding.top);
      let size = view.size();
      let size = vector(
         size.x - padding.left - padding.right,
         size.y - padding.top - padding.bottom,
      );
      let mut new_view = View::new((size.x, size.y));
      new_view.position = position;
      new_view.size = Some(size);
      new_view
   }

   /// Lays the view out as if it filled the whole screen.
   ///
   /// Its position is set to `(0.0, 0.0)`, and its size is set to its constant dimensions.
   ///
   /// The view's dimensions must be Constant.
   pub fn full_screen(view: &mut View) {
      let Dimensions {
         horizontal,
         vertical,
      } = view.dimensions;
      if let Dimension::Constant(width) = horizontal {
         if let Dimension::Constant(height) = vertical {
            view.position = point(0.0, 0.0);
            view.size = Some(vector(width, height));
            return;
         }
      }
      panic!("the dimensions of the view passed to full_screen must be Constant");
   }

   /// Lays the view out such that the given view is aligned inside of the parent view.
   ///
   /// The parent view's size must be computed.
   pub fn align(parent_view: &View, view: &mut View, alignment: Alignment) {
      let parent_size = parent_view.size();
      let size = vector(
         view.dimensions.horizontal.compute(0.0, parent_size.x),
         view.dimensions.vertical.compute(0.0, parent_size.y),
      );
      view.size = Some(size);
      view.position.x = match alignment.0 {
         AlignH::Left => parent_view.position.x,
         AlignH::Center => parent_view.position.x + parent_view.width() / 2.0 - view.width() / 2.0,
         AlignH::Right => parent_view.position.x + parent_view.width() - view.width(),
      };
      view.position.y = match alignment.1 {
         AlignV::Top => parent_view.position.y,
         AlignV::Middle => {
            parent_view.position.y + parent_view.height() / 2.0 - view.height() / 2.0
         }
         AlignV::Bottom => parent_view.position.y + parent_view.height() - view.height(),
      }
   }

   /// Vertical layout direction.
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum DirectionV {
      /// Views are laid out from bottom to top.
      BottomToTop,
   }

   /// Lays out the provided views vertically, in the provided direction.
   ///
   /// The parent view's size must be computed.
   pub fn vertical(parent_view: &View, views: &mut [&mut View], direction: DirectionV) {
      let parent_size = parent_view.size();
      let mut cursor = 0.0;
      for view in views {
         let width = view.dimensions.horizontal.compute(0.0, parent_size.x);
         let height = view.dimensions.vertical.compute(cursor, parent_size.y);
         view.position = point(
            0.0,
            match direction {
               DirectionV::BottomToTop => parent_size.y - cursor - height,
            },
         );
         view.size = Some(vector(width, height));
         cursor += height;
      }
   }
}
