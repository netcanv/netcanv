//! Overflow menu actions.

mod save_to_file;

pub use save_to_file::*;

use crate::assets::Assets;
use crate::backend::{Backend, Image};
use crate::paint_canvas::PaintCanvas;
use crate::project_file::ProjectFile;

pub trait Action {
   /// Returns the name of the action.
   fn name(&self) -> &str;

   /// Returns the icon of the action.
   fn icon(&self) -> &Image;

   /// Performs the action.
   fn perform(&mut self, args: ActionArgs) -> netcanv::Result<()>;

   /// Ticks the action. Called every frame to do things like autosaving.
   fn process(&mut self, ActionArgs { .. }: ActionArgs) -> netcanv::Result<()> {
      Ok(())
   }
}

#[non_exhaustive]
pub struct ActionArgs<'a> {
   pub assets: &'a Assets,
   pub paint_canvas: &'a mut PaintCanvas,
   pub project_file: &'a mut ProjectFile,
   pub renderer: &'a mut Backend,
}

fn _action_trait_must_be_object_safe(_action: Box<dyn Action>) {}
