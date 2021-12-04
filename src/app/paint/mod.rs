//! The paint state. This is the screen where you paint on the canvas with other people.

mod actions;
mod tools;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use netcanv_protocol::matchmaker::PeerId;
use netcanv_renderer::paws::{
   point, vector, AlignH, AlignV, Alignment, Color, Layout, Rect, Renderer, Vector,
};
use netcanv_renderer::{BlendMode, Font, RenderBackend};
use nysa::global as bus;

use crate::app::paint::actions::ActionArgs;
use crate::app::paint::tools::KeyShortcutAction;
use crate::app::*;
use crate::assets::*;
use crate::backend::Backend;
use crate::clipboard;
use crate::common::*;
use crate::config::{ToolbarPosition, UserConfig};
use crate::net::peer::{self, Peer};
use crate::net::timer::Timer;
use crate::paint_canvas::*;
use crate::ui::view::layout::DirectionV;
use crate::ui::view::{Dimension, View};
use crate::ui::wm::WindowManager;
use crate::ui::*;
use crate::viewport::Viewport;

use self::actions::SaveToFileAction;
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

   actions: Vec<Box<dyn actions::Action>>,
   save_to_file: Option<PathBuf>,
   last_autosave: Instant,

   peer: Peer,
   update_timer: Timer,
   chunk_downloads: HashMap<(i32, i32), ChunkDownload>,

   fatal_error: bool,
   log: Log,
   tip: Tip,

   panning: bool,
   viewport: Viewport,

   canvas_view: View,
   bottom_bar_view: View,
   toolbar_view: View,

   overflow_menu: ContextMenu,
   wm: WindowManager,
}

macro_rules! log {
   ($log:expr, $($arg:tt)*) => {
      $log.push((format!($($arg)*), Instant::now()))
   };
}

impl State {
   /// The network communication tick interval.
   pub const TIME_PER_UPDATE: Duration = Duration::from_millis(50);

   /// The height of the bottom bar.
   const BOTTOM_BAR_SIZE: f32 = 32.0;
   /// The width of the toolbar.
   const TOOLBAR_SIZE: f32 = 40.0;
   /// The width and height of a tool button.
   const TOOL_SIZE: f32 = Self::TOOLBAR_SIZE - 8.0;

   /// The amount of padding applied around the canvas area, when laying out elements on top of it.
   const CANVAS_INNER_PADDING: f32 = 8.0;

   /// Creates a new paint state.
   pub fn new(
      assets: Assets,
      config: UserConfig,
      peer: Peer,
      image_path: Option<PathBuf>,
      renderer: &mut Backend,
   ) -> Result<Self, (anyhow::Error, Assets, UserConfig)> {
      let mut this = Self {
         assets,
         config,

         paint_canvas: PaintCanvas::new(),
         tools: Rc::new(RefCell::new(Vec::new())),
         tools_by_name: HashMap::new(),
         current_tool: 0,

         actions: Vec::new(),

         peer,
         update_timer: Timer::new(Self::TIME_PER_UPDATE),
         chunk_downloads: HashMap::new(),

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

         canvas_view: View::new((Dimension::Percentage(1.0), Dimension::Rest(1.0))),
         bottom_bar_view: View::new((Dimension::Percentage(1.0), Self::BOTTOM_BAR_SIZE)),
         toolbar_view: View::new((Self::TOOLBAR_SIZE, 0.0)),

         overflow_menu: ContextMenu::new((256.0, 0.0)), // Vertical is filled in later
         wm: WindowManager::new(),
      };
      this.register_tools(renderer);
      this.register_actions(renderer);

      if let Some(path) = image_path {
         if let Err(error) = this.paint_canvas.load(renderer, &path) {
            return Err((error, this.assets, this.config));
         }
      }

      if this.peer.is_host() {
         log!(this.log, "Welcome to your room!");
         log!(
            this.log,
            "To invite friends, send them the room ID shown in the bottom right corner of your screen."
         );
      }

      Ok(this)
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

   /// Registers all the actions and calculates the layout height of the overflow menu.
   fn register_actions(&mut self, renderer: &mut Backend) {
      self.actions.push(Box::new(SaveToFileAction::new(renderer)));

      let room_id_height = 108.0;
      let separator_height = 8.0 * 2.0;
      let action_height = 32.0;
      let action_margin = 4.0;
      let actions_height = action_height * self.actions.len() as f32
         + action_margin * (self.actions.len() - 1) as f32
         + 4.0;
      self.overflow_menu.view.dimensions.vertical =
         Dimension::Constant(room_id_height + separator_height + actions_height);
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
            wm: &mut self.wm,
            canvas_view: &self.canvas_view,
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
               wm: &mut self.wm,
               canvas_view: &self.canvas_view,
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
      self.canvas_view.begin(ui, input, Layout::Freeform);
      let canvas_size = ui.size();

      //
      // Input
      //

      // Panning and zooming

      match input.action(MouseButton::Middle) {
         (true, ButtonState::Pressed) if ui.hover(input) => self.panning = true,
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
               wm: &mut p.wm,
               canvas_view: &p.canvas_view,
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
                        wm: &mut self.wm,
                        canvas_view: &self.canvas_view,
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
                  wm: &mut p.wm,
                  canvas_view: &p.canvas_view,
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

      self.canvas_view.end(ui);

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
      self.bottom_bar_view.begin(ui, input, Layout::Horizontal);

      ui.fill(self.assets.colors.panel);
      ui.pad((8.0, 0.0));

      // Tool

      self.with_current_tool(|p, tool| {
         tool.process_bottom_bar(ToolArgs {
            ui,
            input,
            wm: &mut p.wm,
            canvas_view: &p.canvas_view,
            assets: &p.assets,
            net: Net::new(&mut p.peer),
         });
      });

      //
      // Right side
      // Note that elements in HorizontalRev go from right to left rather than left to right.
      //

      // TODO: move this to an overflow menu

      ui.push((ui.remaining_width(), ui.height()), Layout::HorizontalRev);

      if Button::with_icon(
         ui,
         input,
         ButtonArgs {
            height: ui.height(),
            colors: &self.assets.colors.action_button,
            corner_radius: 0.0,
         },
         &self.assets.icons.navigation.menu,
      )
      .clicked()
      {
         self.overflow_menu.toggle();
      }

      ui.pop();

      self.bottom_bar_view.end(ui);
   }

   /// Processes the overflow menu.
   fn process_overflow_menu(&mut self, ui: &mut Ui, input: &mut Input) {
      if self
         .overflow_menu
         .begin(
            ui,
            input,
            ContextMenuArgs {
               colors: &self.assets.colors.context_menu,
            },
         )
         .is_open()
      {
         ui.pad(8.0);

         // Room ID display

         ui.push((ui.width(), 0.0), Layout::Vertical);
         ui.pad((8.0, 0.0));
         ui.space(8.0);

         ui.vertical_label(
            &self.assets.sans,
            "Room ID",
            self.assets.colors.text,
            AlignH::Left,
         );
         ui.space(8.0);

         let id_text = format!("{}", self.peer.room_id().unwrap());
         ui.push((ui.width(), 32.0), Layout::HorizontalRev);
         if Button::with_icon(
            ui,
            input,
            ButtonArgs {
               height: ui.height(),
               colors: &self.assets.colors.action_button,
               corner_radius: 0.0,
            },
            &self.assets.icons.navigation.copy,
         )
         .clicked()
         {
            log!(self.log, "Room ID copied to clipboard");
            catch!(clipboard::copy_string(id_text.clone()));
         }
         ui.horizontal_label(
            &self.assets.monospace.with_size(24.0),
            &id_text,
            self.assets.colors.text,
            Some(ui.remaining_width()),
         );
         ui.pop();

         ui.fit();
         ui.pop();
         ui.space(4.0);

         // Room host display

         ui.push((ui.width(), 32.0), Layout::Horizontal);
         ui.icon(
            if self.peer.is_host() {
               &self.assets.icons.peer.host
            } else {
               &self.assets.icons.peer.client
            },
            self.assets.colors.text,
            Some(vector(ui.height(), ui.height())),
         );
         ui.space(4.0);
         if self.peer.is_host() {
            ui.horizontal_label(
               &self.assets.sans,
               "You are the host",
               self.assets.colors.text,
               None,
            );
         } else {
            ui.push(
               (ui.remaining_width(), self.assets.sans.height() * 2.0 + 4.0),
               Layout::Vertical,
            );
            ui.align((AlignH::Right, AlignV::Middle));
            let name = truncate_text(
               &self.assets.sans_bold,
               ui.width(),
               self.peer.host_name().unwrap_or("<unknown>"),
            );
            ui.vertical_label(
               &self.assets.sans_bold,
               &name,
               self.assets.colors.text,
               AlignH::Left,
            );
            ui.space(4.0);
            ui.vertical_label(
               &self.assets.sans,
               "is your host",
               self.assets.colors.text,
               AlignH::Left,
            );
            ui.pop();
         }
         ui.pop();

         ui.space(8.0);
         ui.push((ui.width(), 0.0), Layout::Freeform);
         ui.border_top(self.assets.colors.separator, 1.0);
         ui.pop();
         ui.space(8.0);

         for action in &mut self.actions {
            if Button::process(
               ui,
               input,
               ButtonArgs {
                  height: 32.0,
                  colors: &self.assets.colors.action_button,
                  corner_radius: 2.0,
               },
               Some(ui.width()),
               |ui| {
                  ui.push(ui.size(), Layout::Horizontal);
                  ui.icon(
                     action.icon(),
                     self.assets.colors.text,
                     Some(vector(ui.height(), ui.height())),
                  );
                  ui.space(4.0);
                  ui.horizontal_label(
                     &self.assets.sans,
                     action.name(),
                     self.assets.colors.text,
                     None,
                  );
                  ui.pop();
               },
            )
            .clicked()
            {
               if let Err(error) = action.perform(ActionArgs {
                  paint_canvas: &mut self.paint_canvas,
               }) {
                  log!(self.log, "error while performing action: {}", error);
               }
            }
            ui.space(4.0);
         }

         self.overflow_menu.end(ui);
      }
   }

   /// Reflows the toolbar's size.
   fn resize_toolbar(&mut self) {
      let length = 4.0 + self.tools.borrow().len() as f32 * (Self::TOOL_SIZE + 4.0);
      self.toolbar_view.dimensions = match self.config.ui.toolbar_position {
         ToolbarPosition::Left | ToolbarPosition::Right => (Self::TOOLBAR_SIZE, length),
         ToolbarPosition::Top | ToolbarPosition::Bottom => (length, Self::TOOLBAR_SIZE),
      }
      .into();
   }

   /// Returns the toolbar's alignment inside the canvas view.
   fn toolbar_alignment(&self) -> Alignment {
      match self.config.ui.toolbar_position {
         ToolbarPosition::Left => (AlignH::Left, AlignV::Middle),
         ToolbarPosition::Right => (AlignH::Right, AlignV::Middle),
         ToolbarPosition::Top => (AlignH::Center, AlignV::Top),
         ToolbarPosition::Bottom => (AlignH::Center, AlignV::Bottom),
      }
   }

   /// Processes the toolbar.
   fn process_toolbar(&mut self, ui: &mut Ui, input: &mut Input) {
      self.toolbar_view.begin(ui, input, Layout::Vertical);

      ui.fill_rounded(self.assets.colors.panel, ui.width().min(ui.height()) / 2.0);
      ui.pad(4.0);

      let tools = self.tools.borrow_mut();
      let mut selected_tool = None;
      for (i, tool) in tools.iter().enumerate() {
         ui.push((Self::TOOL_SIZE, Self::TOOL_SIZE), Layout::Freeform);
         if Button::with_icon(
            ui,
            input,
            ButtonArgs {
               height: Self::TOOL_SIZE,
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

      self.toolbar_view.end(ui);
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
         MessageKind::NewHost(name) => log!(self.log, "{} is now hosting the room", name),
         MessageKind::NowHosting => log!(self.log, "You are now hosting the room"),
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

   fn reflow_layout(&mut self, root_view: &View) -> () {
      // The bottom bar and the canvas.
      view::layout::vertical(
         root_view,
         &mut [&mut self.bottom_bar_view, &mut self.canvas_view],
         DirectionV::BottomToTop,
      );
      let padded_canvas = view::layout::padded(&self.canvas_view, Self::CANVAS_INNER_PADDING);

      // The toolbar.
      self.resize_toolbar();
      let toolbar_alignment = self.toolbar_alignment();
      view::layout::align(&padded_canvas, &mut self.toolbar_view, toolbar_alignment);

      // The overflow menu.
      view::layout::align(
         &padded_canvas,
         &mut self.overflow_menu.view,
         (AlignH::Right, AlignV::Bottom),
      );
   }
}

impl AppState for State {
   fn process(
      &mut self,
      StateArgs {
         ui,
         input,
         root_view,
      }: StateArgs,
   ) {
      ui.clear(Color::WHITE);

      // Autosaving

      for action in &mut self.actions {
         match action.process(ActionArgs {
            paint_canvas: &mut self.paint_canvas,
         }) {
            Ok(()) => (),
            Err(error) => log!(self.log, "error while processing action: {}", error),
         }
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

      // Layout
      self.reflow_layout(&root_view);

      // Paint canvas
      self.process_canvas(ui, input);

      // Bars
      self.process_toolbar(ui, input);
      // Draw windows over the toolbar, but below the bottom bar.
      self.wm.process(ui, input, &self.assets);
      self.process_bar(ui, input);
      self.process_overflow_menu(ui, input);
   }

   fn next_state(self: Box<Self>, _renderer: &mut Backend) -> Box<dyn AppState> {
      if self.fatal_error {
         Box::new(lobby::State::new(self.assets, self.config))
      } else {
         self
      }
   }
}
