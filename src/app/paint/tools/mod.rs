//! Painting tools - brushes, selections, and all the like.

use std::net::SocketAddr;
use std::ops::Deref;

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

   /// Called to render a peer on the paint canvas.
   fn process_paint_canvas_peer(
      &mut self,
      _args: ToolArgs,
      _viewport: &Viewport,
      _address: SocketAddr,
   ) {
   }

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
   /// This can be used to initialize the tool's state for the peer.
   fn network_peer_activate(&mut self, _net: Net, _address: SocketAddr) -> anyhow::Result<()> {
      Ok(())
   }

   /// Called when a peer has selected this tool.
   ///
   /// This can be used to write back the changes a peer was in the middle of doing, but didn't
   /// finish before switching to another tool.
   fn network_peer_deactivate(
      &mut self,
      _renderer: &mut Backend,
      _net: Net,
      _paint_canvas: &mut PaintCanvas,
      _address: SocketAddr,
   ) -> anyhow::Result<()> {
      Ok(())
   }
}

pub struct Net<'peer> {
   pub peer: &'peer Peer,
}

impl<'peer> Net<'peer> {
   /// Creates a new `Net` for the given peer.
   pub fn new(peer: &'peer Peer) -> Net {
      Self { peer }
   }

   /// Sends a tool packet.
   pub fn send<T>(&self, tool: &impl Tool, payload: T) -> anyhow::Result<()>
   where
      T: 'static + Serialize,
   {
      let payload = bincode::serialize(&payload)?;
      self.peer.send_tool(tool.name().to_owned(), payload)?;
      Ok(())
   }

   /// Returns the name of the given peer, if the peer is present.
   pub fn peer_name(&self, address: SocketAddr) -> Option<&str> {
      self.peer.mates().get(&address).map(|mate| mate.nickname.deref())
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
