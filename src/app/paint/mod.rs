//! The paint state. This is the screen where you paint on the canvas with other people.

mod tools;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use native_dialog::FileDialog;
use netcanv_protocol::matchmaker::PeerId;
use netcanv_renderer::paws::{
   point, vector, AlignH, AlignV, Color, Layout, Rect, Renderer, Vector,
};
use netcanv_renderer::{BlendMode, Font, RenderBackend};
use nysa::global as bus;

use crate::app::paint::tools::KeyShortcutAction;
use crate::app::*;
use crate::assets::*;
use crate::backend::Backend;
use crate::common::*;
use crate::config::{ToolbarPosition, UserConfig};
use crate::net::peer::{self, Peer};
use crate::net::timer::Timer;
use crate::paint_canvas::*;
use crate::ui::*;
use crate::viewport::Viewport;

use self::tools::{BrushTool, Net, SelectionTool, Tool, ToolArgs};

/// A log message in the lower left corner.
///
/// These are used for displaying errors and joined/left messages.
type Log = Vec<(String, Instant)>;

/// A small tip in the upper left corner.
///
/// These are used for displaying the panning and zoom level.
struct Tip {
   text: String,
   created: Instant,
   visible_duration: Duration,
}

/// The state of a chunk download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkDownload {
   NotDownloaded,
   Queued,
   Requested,
   Downloaded,
}

/// A bus message requesting a chunk download.
struct RequestChunkDownload((i32, i32));

/// The paint app state.
pub struct State {
   assets: Assets,
   config: UserConfig,

   paint_canvas: PaintCanvas,
   tools: Rc<RefCell<Vec<Box<dyn Tool>>>>,
   tools_by_name: HashMap<String, usize>,
   current_tool: usize,

   peer: Peer,
   update_timer: Timer,
   chunk_downloads: HashMap<(i32, i32), ChunkDownload>,

   load_from_file: Option<PathBuf>,
   save_to_file: Option<PathBuf>,
   last_autosave: Instant,

   fatal_error: bool,
   log: Log,
   tip: Tip,

   panning: bool,
   viewport: Viewport,
}

macro_rules! log {
   ($log:expr, $($arg:tt)*) => {
      $log.push((format!($($arg)*), Instant::now()))
   };
}

impl State {
   /// The interval of automatic saving.
   const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(3 * 60);
   /// The height of the bottom bar.
   const BOTTOM_BAR_SIZE: f32 = 32.0;
   /// The width of the toolbar.
   const TOOLBAR_SIZE: f32 = 40.0;
   /// The network communication tick interval.
   pub const TIME_PER_UPDATE: Duration = Duration::from_millis(50);

   /// Creates a new paint state.
   pub fn new(
      assets: Assets,
      config: UserConfig,
      peer: Peer,
      image_path: Option<PathBuf>,
      renderer: &mut Backend,
   ) -> Self {
      let mut this = Self {
         assets,
         config,

         paint_canvas: PaintCanvas::new(),
         tools: Rc::new(RefCell::new(Vec::new())),
         tools_by_name: HashMap::new(),
         current_tool: 0,

         peer,
         update_timer: Timer::new(Self::TIME_PER_UPDATE),
         chunk_downloads: HashMap::new(),

         load_from_file: image_path,
         save_to_file: None,
         last_autosave: Instant::now(),

         fatal_error: false,
         log: Log::new(),
         tip: Tip {
            text: "".into(),
            created: Instant::now(),
            visible_duration: Default::default(),
         },

         panning: false,
         viewport: Viewport::new(),
      };
      this.register_tools(renderer);

      if this.peer.is_host() {
         log!(this.log, "Welcome to your room!");
         log!(
            this.log,
            "To invite friends, send them the room ID shown in the bottom right corner of your screen."
         );
      }

      this
   }

   /// Registers a tool.
   fn register_tool(&mut self, tool: Box<dyn Tool>) {
      let mut tools = self.tools.borrow_mut();
      self.tools_by_name.insert(tool.name().to_owned(), tools.len());
      tools.push(tool);
   }

   /// Registers all the tools.
   fn register_tools(&mut self, renderer: &mut Backend) {
      self.register_tool(Box::new(SelectionTool::new(renderer)));
      // Set the default tool to the brush.
      self.current_tool = self.tools.borrow().len();
      self.register_tool(Box::new(BrushTool::new(renderer)));
   }

   /// Executes the given callback with the currently selected tool.
   fn with_current_tool<R>(
      &mut self,
      mut callback: impl FnMut(&mut Self, &mut Box<dyn Tool>) -> R,
   ) -> R {
      let tools = Rc::clone(&self.tools);
      let mut tools = tools.borrow_mut();
      let tool = &mut tools[self.current_tool];
      callback(self, tool)
   }

   /// Sets the current tool to the one with the provided ID.
   fn set_current_tool(&mut self, renderer: &mut Backend, tool: usize) {
      let previous_tool = self.current_tool;
      let mut tools = self.tools.borrow_mut();
      if tool != previous_tool {
         tools[previous_tool].deactivate(renderer, &mut self.paint_canvas);
         catch!(self.peer.send_select_tool(tools[tool].name().to_owned()));
      }
      tools[tool].activate();
      self.current_tool = tool;
   }

   /// Clones the current tool's name into a String.
   fn clone_tool_name(&self) -> String {
      self.tools.borrow()[self.current_tool].name().to_owned()
   }

   /// Requests a chunk download from the host.
   fn queue_chunk_download(chunk_position: (i32, i32)) {
      bus::push(RequestChunkDownload(chunk_position));
   }

   /// Shows a tip in the upper left corner.
   fn show_tip(&mut self, text: &str, duration: Duration) {
      self.tip = Tip {
         text: text.into(),
         created: Instant::now(),
         visible_duration: duration,
      };
   }

   /// Decodes canvas data to the given chunk.
   fn canvas_data(&mut self, ui: &mut Ui, chunk_position: (i32, i32), image_data: &[u8]) {
      catch!(self.paint_canvas.decode_network_data(ui.render(), chunk_position, image_data));
   }

   /// Processes the message log.
   fn process_log(&mut self, ui: &mut Ui) {
      self.log.retain(|(_, time_created)| time_created.elapsed() < Duration::from_secs(5));
      ui.draw(|ui| {
         let mut y = ui.height() - (self.log.len() as f32 - 1.0) * 16.0 - 8.0;
         let renderer = ui.render();
         renderer.push();
         renderer.set_blend_mode(BlendMode::Invert);
         for (entry, _) in &self.log {
            renderer.text(
               Rect::new(point(8.0, y), vector(0.0, 0.0)),
               &self.assets.sans,
               &entry,
               Color::WHITE.with_alpha(240),
               (AlignH::Left, AlignV::Bottom),
            );
            y += 16.0;
         }
         renderer.pop();
      });
   }

   fn process_tool_key_shortcuts(&mut self, ui: &mut Ui, input: &mut Input) {
      let mut tools = self.tools.borrow_mut();

      match tools[self.current_tool].active_key_shortcuts(
         ToolArgs {
            ui,
            input,
            assets: &mut self.assets,
            net: Net::new(&self.peer),
         },
         &mut self.paint_canvas,
         &self.viewport,
      ) {
         KeyShortcutAction::None => (),
         KeyShortcutAction::Success => return,
         KeyShortcutAction::SwitchToThisTool => (),
      }

      let mut switch_tool = None;
      'tools: for (i, tool) in tools.iter_mut().enumerate() {
         match tool.global_key_shortcuts(
            ToolArgs {
               ui,
               input,
               assets: &mut self.assets,
               net: Net::new(&self.peer),
            },
            &mut self.paint_canvas,
            &self.viewport,
         ) {
            KeyShortcutAction::None => (),
            KeyShortcutAction::Success => return,
            KeyShortcutAction::SwitchToThisTool => {
               switch_tool = Some(i);
               break 'tools;
            }
         }
      }

      drop(tools);
      if let Some(tool) = switch_tool {
         self.set_current_tool(ui, tool);
      }

      return;
   }

   /// Processes the paint canvas.
   fn process_canvas(&mut self, ui: &mut Ui, input: &mut Input) {
      ui.push(
         (ui.width(), ui.height() - Self::BOTTOM_BAR_SIZE),
         Layout::Freeform,
      );
      input.set_mouse_area(mouse_areas::CANVAS, ui.has_mouse(input));
      let canvas_size = ui.size();

      //
      // Input
      //

      // Panning and zooming

      match input.action(MouseButton::Middle) {
         (true, ButtonState::Pressed) if ui.has_mouse(input) => self.panning = true,
         (_, ButtonState::Released) => self.panning = false,
         _ => (),
      }

      if self.panning {
         let delta_pan = input.previous_mouse_position() - input.mouse_position();
         self.viewport.pan_around(delta_pan);
         let pan = self.viewport.pan();
         let position = format!("{}, {}", (pan.x / 256.0).floor(), (pan.y / 256.0).floor());
         self.show_tip(&position, Duration::from_millis(100));
      }
      if let (true, Some(scroll)) = input.action(MouseScroll) {
         self.viewport.zoom_in(scroll.y);
         self.show_tip(
            &format!("{:.0}%", self.viewport.zoom() * 100.0),
            Duration::from_secs(3),
         );
      }

      // Drawing & key shortcuts

      self.process_tool_key_shortcuts(ui, input);

      self.with_current_tool(|p, tool| {
         tool.process_paint_canvas_input(
            ToolArgs {
               ui,
               input,
               assets: &p.assets,
               net: Net::new(&mut p.peer),
            },
            &mut p.paint_canvas,
            &p.viewport,
         )
      });

      //
      // Rendering
      //

      ui.draw(|ui| {
         ui.render().push();
         let Vector {
            x: width,
            y: height,
         } = ui.size();
         ui.render().translate(vector(width / 2.0, height / 2.0));
         ui.render().scale(vector(self.viewport.zoom(), self.viewport.zoom()));
         ui.render().translate(-self.viewport.pan());
         self.paint_canvas.draw_to(ui.render(), &self.viewport, canvas_size);
         ui.render().pop();

         ui.render().push();
         for (&address, mate) in self.peer.mates() {
            if let Some(tool_name) = &mate.tool {
               if let Some(&tool_id) = self.tools_by_name.get(tool_name) {
                  let mut tools = self.tools.borrow_mut();
                  let tool = &mut tools[tool_id];
                  tool.process_paint_canvas_peer(
                     ToolArgs {
                        ui,
                        input,
                        assets: &self.assets,
                        net: Net::new(&self.peer),
                     },
                     &self.viewport,
                     address,
                  );
               }
            }
         }
         ui.render().pop();

         self.with_current_tool(|p, tool| {
            tool.process_paint_canvas_overlays(
               ToolArgs {
                  ui,
                  input,
                  assets: &p.assets,
                  net: Net::new(&mut p.peer),
               },
               &p.viewport,
            );
         });
      });
      if self.tip.created.elapsed() < self.tip.visible_duration {
         ui.push(ui.size(), Layout::Freeform);
         ui.pad((16.0, 16.0));
         ui.push((72.0, 32.0), Layout::Freeform);
         ui.fill(Color::BLACK.with_alpha(192));
         ui.text(
            &self.assets.sans,
            &self.tip.text,
            Color::WHITE,
            (AlignH::Center, AlignV::Middle),
         );
         ui.pop();
         ui.pop();
      }

      self.process_log(ui);

      ui.pop();

      //
      // Networking
      //

      self.update_timer.tick();
      while self.update_timer.update() {
         // Tool updates
         self.with_current_tool(|p, tool| {
            catch!(tool.network_send(tools::Net { peer: &mut p.peer }))
         });
         // Chunk downloading
         if self.save_to_file.is_some() {
            // FIXME: Regression introduced in 0.3.0: saving does not require all chunks to be
            // downloaded.
            // There's some internal debate I've been having on the topic od downloading all chunks
            // when the user requests a save. The main issue I see is that on large canvases
            // downloading all chunks may stall the host for too long, lagging everything to death.
            // If a client wants to download all the chunks, they should probably just explore
            // enough of the canvas such that all the chunks get loaded.
            catch!(self.paint_canvas.save(Some(&self.save_to_file.as_ref().unwrap())));
            self.last_autosave = Instant::now();
            self.save_to_file = None;
         } else {
            for chunk_position in self.viewport.visible_tiles(Chunk::SIZE, canvas_size) {
               if let Some(state) = self.chunk_downloads.get_mut(&chunk_position) {
                  if *state == ChunkDownload::NotDownloaded {
                     Self::queue_chunk_download(chunk_position);
                     *state = ChunkDownload::Queued;
                  }
               }
            }
         }
      }
   }

   /// Processes the bottom bar.
   fn process_bar(&mut self, ui: &mut Ui, input: &mut Input) {
      ui.push((ui.width(), Self::BOTTOM_BAR_SIZE), Layout::Horizontal);
      ui.align((AlignH::Left, AlignV::Bottom));
      input.set_mouse_area(mouse_areas::BOTTOM_BAR, ui.has_mouse(input));
      ui.fill(self.assets.colors.panel);
      ui.pad((8.0, 0.0));

      // Tool

      self.with_current_tool(|p, tool| {
         tool.process_bottom_bar(ToolArgs {
            ui,
            input,
            assets: &p.assets,
            net: Net::new(&mut p.peer),
         });
      });

      //
      // Right side
      // Note that elements in HorizontalRev go from right to left rather than left to right.
      //

      // Room ID display

      ui.push((ui.remaining_width(), ui.height()), Layout::HorizontalRev);
      if Button::with_icon(
         ui,
         input,
         ButtonArgs {
            height: 32.0,
            colors: &self.assets.colors.action_button,
            corner_radius: 0.0,
         },
         &self.assets.icons.file.save,
      )
      .clicked()
      {
         match FileDialog::new()
            .set_filename("canvas.png")
            .add_filter("PNG image", &["png"])
            .add_filter("NetCanv canvas", &["netcanv", "toml"])
            .show_save_single_file()
         {
            Ok(Some(path)) => {
               self.save_to_file = Some(path);
            }
            Err(error) => log!(self.log, "Error while selecting file: {}", error),
            _ => (),
         }
      }

      // The room ID itself
      let id_text = format!("{}", self.peer.room_id().unwrap());
      ui.push((72.0, ui.height()), Layout::Freeform);
      ui.text(
         &self.assets.monospace.with_size(15.0),
         &id_text,
         self.assets.colors.text,
         (AlignH::Center, AlignV::Middle),
      );
      ui.pop();

      // "Room ID" text
      ui.push((64.0, ui.height()), Layout::Freeform);
      ui.text(
         &self.assets.sans,
         "Room ID",
         self.assets.colors.text,
         (AlignH::Center, AlignV::Middle),
      );
      ui.pop();

      ui.pop();

      ui.pop();
   }

   /// Processes the toolbar.
   fn process_toolbar(&mut self, ui: &mut Ui, input: &mut Input) {
      // The outer group, to add some padding.
      ui.push(
         (ui.width(), ui.height() - Self::BOTTOM_BAR_SIZE),
         Layout::Freeform,
      );
      ui.pad(8.0);

      // The inner group, that actually contains the bar.
      let tool_size = Self::TOOLBAR_SIZE - 8.0;
      let length = 4.0 + self.tools.borrow().len() as f32 * (tool_size + 4.0);
      ui.push(
         match self.config.ui.toolbar_position {
            ToolbarPosition::Left | ToolbarPosition::Right => (Self::TOOLBAR_SIZE, length),
            ToolbarPosition::Top | ToolbarPosition::Bottom => (length, Self::TOOLBAR_SIZE),
         },
         match self.config.ui.toolbar_position {
            ToolbarPosition::Left | ToolbarPosition::Right => Layout::Vertical,
            ToolbarPosition::Top | ToolbarPosition::Bottom => Layout::Horizontal,
         },
      );
      ui.align(match self.config.ui.toolbar_position {
         ToolbarPosition::Left => (AlignH::Left, AlignV::Middle),
         ToolbarPosition::Right => (AlignH::Right, AlignV::Middle),
         ToolbarPosition::Top => (AlignH::Center, AlignV::Top),
         ToolbarPosition::Bottom => (AlignH::Center, AlignV::Bottom),
      });
      input.set_mouse_area(mouse_areas::TOOLBAR, ui.has_mouse(input));
      ui.fill_rounded(self.assets.colors.panel, ui.width().min(ui.height()) / 2.0);
      ui.pad(4.0);

      let tools = self.tools.borrow_mut();

      let mut selected_tool = None;
      for (i, tool) in tools.iter().enumerate() {
         ui.push((tool_size, tool_size), Layout::Freeform);
         if Button::with_icon(
            ui,
            input,
            ButtonArgs {
               height: tool_size,
               colors: if self.current_tool == i {
                  &self.assets.colors.selected_toolbar_button
               } else {
                  &self.assets.colors.toolbar_button
               },
               corner_radius: ui.width() / 2.0,
            },
            tool.icon(),
         )
         .clicked()
         {
            selected_tool = Some(i);
         }
         ui.pop();
         ui.space(4.0);
      }

      drop(tools);
      if let Some(selected_tool) = selected_tool {
         self.set_current_tool(ui, selected_tool);
      }

      ui.pop();

      ui.pop();
   }

   fn process_peer_message(&mut self, ui: &mut Ui, message: peer::Message) -> anyhow::Result<()> {
      use peer::MessageKind;

      match message.kind {
         MessageKind::Joined(nickname, peer_id) => {
            log!(self.log, "{} joined the room", nickname);
            if self.peer.is_host() {
               let positions = self.paint_canvas.chunk_positions();
               self.peer.send_chunk_positions(peer_id, positions)?;
            }
            // Order matters here! The tool selection packet must arrive before the packets sent
            // from the tool's `network_peer_join` event.
            self.peer.send_select_tool(self.clone_tool_name())?;
            self.with_current_tool(|p, tool| tool.network_peer_join(Net::new(&p.peer), peer_id))?;
         }
         MessageKind::Left {
            peer_id,
            nickname,
            last_tool,
         } => {
            log!(self.log, "{} has left", nickname);
            // Make sure the tool they were last using is properly deinitialized.
            if let Some(tool) = last_tool {
               if let Some(&tool_id) = self.tools_by_name.get(&tool) {
                  let mut tools = self.tools.borrow_mut();
                  let tool = &mut tools[tool_id];
                  tool.network_peer_deactivate(
                     ui,
                     Net::new(&mut self.peer),
                     &mut self.paint_canvas,
                     peer_id,
                  )?;
               }
            }
         }
         MessageKind::ChunkPositions(positions) => {
            eprintln!("received {} chunk positions", positions.len());
            for chunk_position in positions {
               self.chunk_downloads.insert(chunk_position, ChunkDownload::NotDownloaded);
            }
            // Make sure we send the tool _after_ adding the requested chunks.
            // This way if something goes wrong here and the function returns Err, at least we
            // will have queued up some chunk downloads at this point.
            self.peer.send_select_tool(self.clone_tool_name())?;
         }
         MessageKind::Chunks(chunks) => {
            eprintln!("received {} chunks", chunks.len());
            for (chunk_position, image_data) in chunks {
               self.canvas_data(ui, chunk_position, &image_data);
               self.chunk_downloads.insert(chunk_position, ChunkDownload::Downloaded);
            }
         }
         MessageKind::GetChunks(requester, positions) => {
            self.send_chunks(requester, &positions)?;
         }
         MessageKind::Tool(sender, name, payload) => {
            if let Some(&tool_id) = self.tools_by_name.get(&name) {
               let mut tools = self.tools.borrow_mut();
               let tool = &mut tools[tool_id];
               tool.network_receive(
                  ui,
                  Net::new(&mut self.peer),
                  &mut self.paint_canvas,
                  sender,
                  payload.clone(),
               )?;
            }
         }
         MessageKind::SelectTool {
            peer_id: address,
            previous_tool,
            tool,
         } => {
            eprintln!("{:?} selected tool {}", address, tool);
            // Deselect the old tool.
            if let Some(tool) = previous_tool {
               if let Some(&tool_id) = self.tools_by_name.get(&tool) {
                  // ↑ still waiting for if_let_chains to get stabilized.
                  let mut tools = self.tools.borrow_mut();
                  let tool = &mut tools[tool_id];
                  tool.network_peer_deactivate(
                     ui,
                     Net::new(&mut self.peer),
                     &mut self.paint_canvas,
                     address,
                  )?;
               }
            }
            // Select the new tool.
            if let Some(&tool_id) = self.tools_by_name.get(&tool) {
               eprintln!(" - valid tool with ID {}", tool_id);
               let mut tools = self.tools.borrow_mut();
               let tool = &mut tools[tool_id];
               tool.network_peer_activate(Net::new(&mut self.peer), address)?;
            }
         }
      }
      Ok(())
   }

   fn send_chunks(&mut self, peer_id: PeerId, positions: &[(i32, i32)]) -> anyhow::Result<()> {
      const KILOBYTE: usize = 1024;
      const MAX_BYTES_PER_PACKET: usize = 128 * KILOBYTE;

      let mut packet = Vec::new();
      let mut bytes_of_image_data = 0;
      for &chunk_position in positions {
         if bytes_of_image_data > MAX_BYTES_PER_PACKET {
            let packet = std::mem::replace(&mut packet, Vec::new());
            bytes_of_image_data = 0;
            self.peer.send_chunks(peer_id, packet)?;
         }
         if let Some(image_data) = self.paint_canvas.network_data(chunk_position) {
            packet.push((chunk_position, image_data.to_owned()));
            bytes_of_image_data += image_data.len();
         }
      }
      self.peer.send_chunks(peer_id, packet)?;

      Ok(())
   }
}

impl AppState for State {
   fn process(&mut self, StateArgs { ui, input }: StateArgs) {
      ui.clear(Color::WHITE);

      // Loading from file

      if self.load_from_file.is_some() {
         catch!(self.paint_canvas.load(ui, &self.load_from_file.take().unwrap()))
      }

      // Autosaving

      if self.paint_canvas.filename().is_some()
         && self.last_autosave.elapsed() > Self::AUTOSAVE_INTERVAL
      {
         eprintln!("autosaving chunks");
         catch!(self.paint_canvas.save(None));
         eprintln!("autosave complete");
         self.last_autosave = Instant::now();
      }

      // Network

      catch!(self.peer.communicate(), as Fatal);
      for message in &bus::retrieve_all::<peer::Message>() {
         if message.token == self.peer.token() {
            catch!(self.process_peer_message(ui, message.consume()));
         }
      }

      let needed_chunks: Vec<_> = bus::retrieve_all::<RequestChunkDownload>()
         .into_iter()
         .map(|message| message.consume().0)
         .collect();
      if needed_chunks.len() > 0 {
         for &chunk_position in &needed_chunks {
            self.chunk_downloads.insert(chunk_position, ChunkDownload::Requested);
         }
         catch!(self.peer.download_chunks(needed_chunks));
      }

      // Error checking

      for message in &bus::retrieve_all::<Error>() {
         let Error(error) = message.consume();
         log!(self.log, "error: {}", error);
      }
      for _ in &bus::retrieve_all::<Fatal>() {
         self.fatal_error = true;
      }

      // Paint canvas
      self.process_canvas(ui, input);

      // Bars
      self.process_toolbar(ui, input);
      self.process_bar(ui, input);
   }

   fn next_state(self: Box<Self>, _renderer: &mut Backend) -> Box<dyn AppState> {
      if self.fatal_error {
         Box::new(lobby::State::new(self.assets, self.config))
      } else {
         self
      }
   }
}

mod mouse_areas {
   pub const CANVAS: u32 = 0;
   pub const BOTTOM_BAR: u32 = 1;
   pub const TOOLBAR: u32 = 2;
}
