//! Painting tools - brushes, selections, and all the like.

use std::net::SocketAddr;

use crate::assets::Assets;
use crate::backend::{Backend, Image};
use crate::net::peer::Peer;
use crate::paint_canvas::PaintCanvas;
use crate::ui::{Input, Ui};
use crate::viewport::Viewport;

mod brush;
mod selection;

pub use brush::*;
pub use selection::*;
use serde::Serialize;

pub trait Tool {
   /// Returns the name of the tool.
   ///
   /// This is usually a constant, but `&self` must be included for the trait to be object-safe.
   fn name(&self) -> &str;

   /// Returns the icon this tool uses.
   fn icon(&self) -> &Image;

   /// Called when the tool is selected.
   fn activate(&mut self) {}

   /// Called when the tool is deselected.
   ///
   /// The paint canvas can be used to finalize ongoing actions, eg. the selection should get
   /// deselected.
   fn deactivate(&mut self, _renderer: &mut Backend, _paint_canvas: &mut PaintCanvas) {}

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

   /// Called when network packets should be sent.
   fn network_send(&mut self, _net: Net) -> anyhow::Result<()> {
      Ok(())
   }

   /// Called for each incoming packet from a specific `sender`.
   fn network_receive(
      &mut self,
      _renderer: &mut Backend,
      _net: Net,
      _paint_canvas: &mut PaintCanvas,
      _sender: SocketAddr,
      _payload: Vec<u8>,
   ) -> anyhow::Result<()> {
      Ok(())
   }

   /// Called when a peer has selected this tool.
   ///
   /// This can initialize the
   fn network_peer_selected(&mut self) -> anyhow::Result<()> {
      Ok(())
   }
}

pub struct Net<'peer> {
   pub peer: &'peer mut Peer,
}

impl<'peer> Net<'peer> {
   pub fn send<T>(&self, tool: &impl Tool, payload: T) -> anyhow::Result<()>
   where
      T: 'static + Serialize,
   {
      let payload = bincode::serialize(&payload)?;
      self.peer.send_tool(tool.name().to_owned(), payload)?;
      Ok(())
   }

   pub fn new(peer: &'peer mut Peer) -> Net {
      Self { peer }
   }
}

#[non_exhaustive]
pub struct ToolArgs<'ui, 'input, 'state> {
   pub ui: &'ui mut Ui,
   pub input: &'input Input,
   pub assets: &'state Assets,
   pub net: Net<'state>,
}

fn _tool_trait_must_be_object_safe(_: Box<dyn Tool>) {}
