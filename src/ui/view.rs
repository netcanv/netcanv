//! Views and view layouting.

use netcanv_renderer::paws::{point, vector, Layout, Point, Vector};

use crate::token::Token;

use super::{Input, Ui, UiInput};

/// A size.
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

/// A view.
pub struct View {
   /// The ID of the view. This is used for specifying mouse areas in the input layer.
   id: usize,
   /// The position of the view.
   pub position: Point,
   /// The dimensions of the view.
   pub dimensions: (Dimension, Dimension),
   /// The computed size of the view.
   size: Vector,
}

static MOUSE_AREA: Token = Token::new();

impl View {
   /// Creates a new view with the provided dimensions.
   ///
   /// The initial position of the view is `(0.0, 0.0)`.
   pub fn new(dimensions: (Dimension, Dimension)) -> Self {
      Self {
         id: MOUSE_AREA.next(),
         position: point(0.0, 0.0),
         dimensions,
         size: vector(0.0, 0.0),
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
   fn begin(&self, ui: &mut Ui, input: &mut Input, layout: Layout) {
      // Push an intermediary, zero-sized group onto the stack, such that the current group's
      // layout does not get displaced.
      ui.push((0.0, 0.0), Layout::Freeform);
      ui.push(self.size, layout);
      ui.set_position(self.position);
      input.set_mouse_area(self.id, ui.has_mouse(input));
   }

   /// Ends rendering inside the view.
   fn end(&self, ui: &mut Ui) {
      ui.pop();
      ui.pop();
   }
}

/// Functions for laying out views.
pub mod layout {
   use super::*;

   /// Lays the view out as if it filled the whole screen.
   ///
   /// Its position is set to `(0.0, 0.0)`, and its size is set to its constant dimensions.
   ///
   /// The view's dimensions must be Constant.
   pub fn full_screen(view: &mut View) {
      let (width, height) = view.dimensions;
      if let Dimension::Constant(width) = width {
         if let Dimension::Constant(height) = height {
            view.position = point(0.0, 0.0);
            view.size = vector(width, height);
            return;
         }
      }
      panic!("the dimensions of the view passed to full_screen must be Constant");
   }

   /// Vertical layout direction.
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum VDirection {
      /// Views are laid out from top to bottom.
      TopToBottom,
      /// Views are laid out from bottom to top.
      BottomToTop,
   }

   pub use VDirection::*;

   /// Lays out the provided views vertically.
   pub fn vertical(direction: VDirection, parent_view: &View, views: &mut [&mut View]) {
      let width = parent_view.size.x;
      let mut cursor = 0.0;
      for view in views {
         let height = view.dimensions.1.compute(cursor, parent_view.size.y);
         view.position = point(
            0.0,
            match direction {
               TopToBottom => cursor,
               BottomToTop => parent_view.size.y - cursor - height,
            },
         );
         view.size = vector(width, height);
         cursor += height;
      }
   }
}
