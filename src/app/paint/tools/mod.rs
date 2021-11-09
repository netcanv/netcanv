//! Painting tools - brushes, selections, and all the like.

use crate::assets::Assets;
use crate::backend::{Backend, Image};
use crate::paint_canvas::PaintCanvas;
use crate::ui::{Input, Ui};
use crate::viewport::Viewport;

mod brush;

pub use brush::*;

pub trait Tool {
   /// Returns the name of the tool.
   ///
   /// This is usually a constant, but `&self` must be included for the trait to be object-safe.
   fn name(&self) -> &str;

   /// Returns the icon this tool uses.
   fn icon(&self) -> &Image;

   /// Called before the paint canvas is rendered to the screen. Primarily used for drawing to the
   /// paint canvas, or updating related state.
   ///
   /// Should not be used for drawing to the screen, as all effects of drawing would be overwritten
   /// by the canvas itself.
   fn process_paint_canvas_input(
      &mut self,
      _args: ToolArgs,
      _paint_canvas: &mut PaintCanvas,
      _viewport: &Viewport,
   ) {
   }

   /// Called after the paint canvas has been drawn, with panning and zooming applied.
   ///
   /// Can be used for drawing extra layers, shapes, etc. to the screen, hence the name, `layers`.
   ///
   /// The UI state is not available in this callback; rather, raw draw calls have to be used.
   fn process_paint_canvas_layers(
      &mut self,
      _renderer: &mut Backend,
      _input: &Input,
      _viewport: &Viewport,
   ) {
   }

   /// Called after the paint canvas has been drawn, with panning applied, but not zooming.
   ///
   /// The viewport may be used to figure out where to draw specific elements, as well as scaling
   /// for things like drawing the brush cursor.
   fn process_paint_canvas_overlays(&mut self, _args: ToolArgs, _viewport: &Viewport) {}

   /// Called to draw widgets on the bottom bar.
   ///
   /// Each tool can have its own set of widgets for controlling how the tool is used.
   /// For example, the brush tool exposes the color palette and brush size slider.
   ///
   /// If there isn't anything to control, the bottom bar can be used as a status bar for displaying
   /// relevant information, eg. selection size.
   fn process_bottom_bar(&mut self, _args: ToolArgs) {}
}

pub struct ToolArgs<'ui, 'input, 'state> {
   pub ui: &'ui mut Ui,
   pub input: &'input Input,
   pub assets: &'state Assets,
}

fn _tool_trait_must_be_object_safe(_: Box<dyn Tool>) {}
