//! Overflow menu actions.

mod save_to_file;

pub use save_to_file::*;

use crate::backend::Image;
use crate::paint_canvas::PaintCanvas;

pub trait Action {
   /// Returns the name of the action.
   fn name(&self) -> &str;

   /// Returns the icon of the action.
   fn icon(&self) -> &Image;

   /// Performs the action.
   fn perform(&mut self, args: ActionArgs) -> anyhow::Result<()>;

   /// Ticks the action. Called every frame to do things like autosaving.
   fn process(&mut self, ActionArgs { .. }: ActionArgs) -> anyhow::Result<()> {
      Ok(())
   }
}

#[non_exhaustive]
pub struct ActionArgs<'p> {
   pub paint_canvas: &'p mut PaintCanvas,
}

fn _action_trait_must_be_object_safe(_action: Box<dyn Action>) {}
