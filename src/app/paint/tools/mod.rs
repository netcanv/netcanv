//! Painting tools - brushes, selections, and all the like.

use crate::backend::Image;

mod brush;

pub use brush::*;

pub trait Tool {
   /// Returns the name of the tool.
   ///
   /// This is usually a constant, but `&self` must be included for the trait to be object-safe.
   fn name(&self) -> &str;

   /// Returns the icon this tool uses.
   fn icon(&self) -> &Image;
}

fn _tool_trait_must_be_object_safe(_: Box<dyn Tool>) {}
