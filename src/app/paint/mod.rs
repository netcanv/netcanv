//! The paint state. This is the screen where you paint on the canvas with other people.

mod actions;
pub mod tool_bar;
mod tools;

use image::RgbaImage;
use instant::{Duration, Instant};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use netcanv_i18n::translate_enum::TranslateEnum;
use netcanv_protocol::relay::PeerId;
use netcanv_renderer::paws::{
   point, vector, AlignH, AlignV, Color, Layout, Rect, Renderer, Vector,
};
use netcanv_renderer::{BlendMode, Font, RenderBackend};
use nysa::global as bus;
use tokio::sync::mpsc;

use crate::app::paint::actions::ActionArgs;
use crate::app::paint::tool_bar::ToolbarArgs;
use crate::app::paint::tools::KeyShortcutAction;
use crate::app::*;
use crate::assets::*;
use crate::backend::Backend;
use crate::clipboard;
use crate::common::*;
use crate::image_coder::ImageCoder;
use crate::net::peer::{self, Peer};
use crate::net::socket::SocketSystem;
use crate::net::timer::Timer;
use crate::paint_canvas::cache_layer::{CacheLayer, CachedChunk};
use crate::paint_canvas::chunk::Chunk;
use crate::paint_canvas::*;
use crate::project_file::ProjectFile;
use crate::ui::view::layout::DirectionV;
use crate::ui::view::{Dimension, View};
use crate::ui::wm::WindowManager;
use crate::ui::*;
use crate::viewport::Viewport;

use self::actions::SaveToFileAction;
use self::tool_bar::{ToolId, Toolbar};
use self::tools::{BrushTool, EyedropperTool, Net, SelectionTool, ToolArgs};

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

/// Controls shared between tools, such as the color palette.
pub struct GlobalControls {
   pub color_picker: ColorPicker,
}

struct EncodeChannels {
   tx: mpsc::UnboundedSender<((i32, i32), CachedChunk)>,
   rx: mpsc::UnboundedReceiver<((i32, i32), CachedChunk)>,
}

struct DecodeChannels {
   tx: mpsc::UnboundedSender<((i32, i32), RgbaImage)>,
   rx: mpsc::UnboundedReceiver<((i32, i32), RgbaImage)>,
}

/// The paint app state.
pub struct State {
   assets: Box<Assets>,
   socket_system: Arc<SocketSystem>,
   project_file: ProjectFile,

   paint_canvas: PaintCanvas,
   cache_layer: CacheLayer,

   actions: Vec<Box<dyn actions::Action>>,

   peer: Peer,
   update_timer: Timer,
   chunk_downloads: HashMap<(i32, i32), ChunkDownload>,
   encoded_chunks: HashMap<PeerId, EncodeChannels>,
   encode_channels: EncodeChannels,
   decode_channels: DecodeChannels,

   fatal_error: bool,
   log: Log,
   tip: Tip,

   panning: bool,
   viewport: Viewport,

   canvas_view: View,
   bottom_bar_view: View,

   overflow_menu: ContextMenu,
   toolbar: Toolbar,
   wm: WindowManager,
   global_controls: GlobalControls,
}

macro_rules! log {
   ($log:expr, $($arg:tt)*) => {
      $log.push((format!($($arg)*), Instant::now()))
   };
}

macro_rules! tool_args {
   ($ui:expr, $input:expr, $state:expr) => {
      ToolArgs {
         ui: $ui,
         input: $input,
         wm: &mut $state.wm,
         canvas_view: &$state.canvas_view,
         global_controls: &mut $state.global_controls,
         assets: &mut $state.assets,
         net: Net::new(&$state.peer),
      }
   };
}

impl State {
   /// The network communication tick interval.
   pub const TIME_PER_UPDATE: Duration = Duration::from_millis(50);

   /// The height of the bottom bar.
   const BOTTOM_BAR_SIZE: f32 = 32.0;

   /// The amount of padding applied around the canvas area, when laying out elements on top of it.
   const CANVAS_INNER_PADDING: f32 = 8.0;

   /// Creates a new paint state.
   pub fn new(
      assets: Box<Assets>,
      socket_system: Arc<SocketSystem>,
      peer: Peer,
      image_path: Option<PathBuf>,
      renderer: &mut Backend,
   ) -> Result<Self, (netcanv::Error, Box<Assets>)> {
      let (encoded_tx, encoded_rx) = mpsc::unbounded_channel();
      let (decoded_tx, decoded_rx) = mpsc::unbounded_channel();

      let mut wm = WindowManager::new();
      let mut this = Self {
         assets,
         socket_system,

         paint_canvas: PaintCanvas::new(),
         cache_layer: CacheLayer::new(),
         project_file: ProjectFile::new(),

         actions: Vec::new(),

         peer,
         update_timer: Timer::new(Self::TIME_PER_UPDATE),
         chunk_downloads: HashMap::new(),
         encoded_chunks: HashMap::new(),
         encode_channels: EncodeChannels {
            tx: encoded_tx,
            rx: encoded_rx,
         },
         decode_channels: DecodeChannels {
            tx: decoded_tx,
            rx: decoded_rx,
         },

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

         overflow_menu: ContextMenu::new((256.0, 0.0)), // Vertical is filled in later
         toolbar: Toolbar::new(&mut wm),
         wm,

         global_controls: GlobalControls {
            color_picker: ColorPicker::new(),
         },
      };
      this.register_tools(renderer);
      this.register_actions(renderer);

      if let Some(path) = image_path {
         if let Err(error) = this.project_file.load(renderer, &path, &mut this.paint_canvas) {
            return Err((error, this.assets));
         }
      }

      if this.peer.is_host() {
         for line in this.assets.tr.paint_welcome_host.split('\n') {
            log!(this.log, "{}", line);
         }
         this.overflow_menu.open();
      }

      Ok(this)
   }

   /// Registers all the tools.
   fn register_tools(&mut self, renderer: &mut Backend) {
      let _selection = self.toolbar.add_tool(SelectionTool::new(renderer));
      let brush = self.toolbar.add_tool(BrushTool::new(renderer));
      let _eyedropper = self.toolbar.add_tool(EyedropperTool::new(renderer));

      // Set the default tool to the brush.
      self.toolbar.set_current_tool(brush);
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

   fn tool_switch_events(
      &mut self,
      renderer: &mut Backend,
      previous_tool: ToolId,
      current_tool: ToolId,
   ) {
      if previous_tool != current_tool {
         self.toolbar.with_tool(previous_tool, |tool| {
            tool.deactivate(renderer, &mut self.paint_canvas);
         });
         catch!(self.peer.send_select_tool(self.toolbar.clone_tool_name(current_tool)));
         self.toolbar.with_tool(current_tool, |tool| tool.activate());
      }
   }

   /// Sets the current tool to the one with the provided ID.
   fn set_current_tool(&mut self, renderer: &mut Backend, tool: ToolId) {
      let previous_tool = self.toolbar.current_tool();
      self.toolbar.set_current_tool(tool);
      self.tool_switch_events(renderer, previous_tool, tool);
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
   fn decode_canvas_data(&mut self, chunk_position: (i32, i32), image_data: Vec<u8>) {
      let tx = self.decode_channels.tx.clone();
      tokio::task::spawn_blocking(move || {
         match ImageCoder::decode_network_data(&image_data) {
            Ok(image) => {
               // Doesn't matter if the receiving half is closed.
               tx.send((chunk_position, image)).expect("Unbounded send failed");
            }
            Err(error) => tracing::error!("image decoding failed: {:?}", error),
         }
      });
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
               entry,
               Color::WHITE.with_alpha(240),
               (AlignH::Left, AlignV::Bottom),
            );
            y += 16.0;
         }
         renderer.pop();
      });
   }

   fn process_tool_key_shortcuts(&mut self, ui: &mut Ui, input: &mut Input) {
      // If any of the WM's windows are focused, skip keyboard shortcuts.
      if self.wm.has_focus() {
         return;
      }

      match self.toolbar.with_current_tool(|tool| {
         tool.active_key_shortcuts(
            tool_args!(ui, input, self),
            &mut self.paint_canvas,
            &self.viewport,
         )
      }) {
         KeyShortcutAction::None => (),
         KeyShortcutAction::Success => return,
         KeyShortcutAction::SwitchToThisTool => (),
      }

      let mut switch_tool = self
         .toolbar
         .with_each_tool(|tool_id, tool| {
            match tool.global_key_shortcuts(
               tool_args!(ui, input, self),
               &mut self.paint_canvas,
               &self.viewport,
            ) {
               KeyShortcutAction::None => (),
               KeyShortcutAction::Success => return ControlFlow::Break(None),
               KeyShortcutAction::SwitchToThisTool => return ControlFlow::Break(Some(tool_id)),
            }
            ControlFlow::Continue
         })
         .flatten();

      self.toolbar.with_each_tool::<(), _>(|tool_id, tool| {
         if input.action(&tool.key_shortcut()) == (true, true) {
            switch_tool = Some(tool_id);
         }
         ControlFlow::Continue
      });

      if let Some(tool) = switch_tool {
         self.set_current_tool(ui, tool);
      }
   }

   /// Processes the paint canvas.
   fn process_canvas(&mut self, ui: &mut Ui, input: &mut Input) {
      self.canvas_view.begin(ui, input, Layout::Freeform);
      let canvas_size = ui.size();

      //
      // Input
      //

      // Panning and zooming

      match input.action(&MouseButton::Middle) {
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
      if let (true, Some(scroll)) = input.action(&MouseScroll) {
         self.viewport.zoom_in(scroll.y);
         self.show_tip(
            &format!("{:.0}%", self.viewport.zoom() * 100.0),
            Duration::from_secs(3),
         );
      }

      // Drawing & key shortcuts

      self.toolbar.with_each_tool::<(), _>(|_, tool| {
         tool.process_background_jobs(tool_args!(ui, input, self), &mut self.paint_canvas);
         ControlFlow::Continue
      });

      self.process_tool_key_shortcuts(ui, input);

      self.toolbar.with_current_tool(|tool| {
         tool.process_paint_canvas_input(
            tool_args!(ui, input, self),
            &mut self.paint_canvas,
            &self.viewport,
         )
      });

      //
      // Rendering
      //

      while let Ok((chunk_position, image)) = self.decode_channels.rx.try_recv() {
         self.paint_canvas.set_chunk(ui, chunk_position, image);
      }
      while let Ok((chunk_position, image)) = self.encode_channels.rx.try_recv() {
         let _ = self.paint_canvas.ensure_chunk(ui, chunk_position);
         self.cache_layer.set_chunk(chunk_position, image);
      }
      self.cache_layer.update_timers();

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
               if let Some(tool_id) = self.toolbar.tool_by_name(tool_name) {
                  self.toolbar.with_tool(tool_id, |tool| {
                     tool.process_paint_canvas_peer(
                        tool_args!(ui, input, self),
                        &self.viewport,
                        address,
                     );
                  });
               }
            }
         }
         ui.render().pop();

         self.toolbar.with_current_tool(|tool| {
            tool.process_paint_canvas_overlays(tool_args!(ui, input, self), &self.viewport);
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
         self.toolbar.with_current_tool(|tool| {
            catch!(tool.network_send(
               tools::Net {
                  peer: &mut self.peer
               },
               &self.global_controls
            ))
         });

         for chunk_position in self.viewport.visible_tiles(Chunk::SIZE, canvas_size) {
            if let Some(state) = self.chunk_downloads.get_mut(&chunk_position) {
               if *state == ChunkDownload::NotDownloaded {
                  Self::queue_chunk_download(chunk_position);
                  *state = ChunkDownload::Queued;
               }
            }
         }

         // Chunk sending
         for (&peer_id, EncodeChannels { rx, .. }) in &mut self.encoded_chunks {
            const KIBIBYTE: usize = 1024;
            const MAX_BYTES_PER_PACKET: usize = 128 * KIBIBYTE;

            let mut bytes_in_packet = 0;
            let mut packet = Vec::new();
            while let Ok((chunk_position, images)) = rx.try_recv() {
               let image_data = match images {
                  CachedChunk {
                     png: _,
                     webp: Some(webp),
                  } => webp,
                  CachedChunk { png, webp: None } => png,
               };
               if bytes_in_packet + image_data.len() > MAX_BYTES_PER_PACKET {
                  catch!(self.peer.send_chunks(peer_id, std::mem::take(&mut packet)));
                  bytes_in_packet = 0;
               }
               bytes_in_packet += image_data.len();
               packet.push((chunk_position, image_data));
            }
            if !packet.is_empty() {
               catch!(self.peer.send_chunks(peer_id, packet));
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

      self.toolbar.with_current_tool(|tool| {
         tool.process_bottom_bar(tool_args!(ui, input, self));
      });

      //
      // Right side
      // Note that elements in HorizontalRev go from right to left rather than left to right.
      //

      ui.push((ui.remaining_width(), ui.height()), Layout::HorizontalRev);

      if Button::with_icon(
         ui,
         input,
         &ButtonArgs::new(ui, &self.assets.colors.action_button),
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
            &self.assets.tr.room_id,
            self.assets.colors.text,
            AlignH::Left,
         );
         ui.space(8.0);

         let id_text = format!("{}", self.peer.room_id().unwrap());
         ui.push((ui.width(), 32.0), Layout::HorizontalRev);
         if Button::with_icon(
            ui,
            input,
            &ButtonArgs::new(ui, &self.assets.colors.action_button).corner_radius(4.0),
            &self.assets.icons.navigation.copy,
         )
         .clicked()
         {
            log!(self.log, "{}", &self.assets.tr.room_id_copied);
            catch!(clipboard::copy_string(id_text.clone()));
         }
         ui.horizontal_label(
            &self.assets.monospace.with_size(24.0),
            &id_text,
            self.assets.colors.text,
            Some((ui.remaining_width(), AlignH::Center)),
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
               &self.assets.tr.you_are_the_host,
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
               self.peer.host_name().unwrap_or(&self.assets.tr.unknown_host),
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
               &self.assets.tr.someone_is_your_host,
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
            let action_button = Button::process(
               ui,
               input,
               &ButtonArgs::new(ui, &self.assets.colors.action_button)
                  .height(32.0)
                  .corner_radius(4.0),
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
                     &self.assets.tr.action.get(action.name()),
                     self.assets.colors.text,
                     None,
                  );
                  ui.pop();
               },
            );
            if action_button.clicked() {
               if let Err(error) = action.perform(ActionArgs {
                  assets: &self.assets,
                  paint_canvas: &mut self.paint_canvas,
                  project_file: &mut self.project_file,
                  renderer: ui,
               }) {
                  log!(
                     self.log,
                     "{}",
                     self
                        .assets
                        .tr
                        .error_while_performing_action
                        .format()
                        .with("error", error.translate(&self.assets.language))
                        .done()
                  );
               }
            }
            ui.space(4.0);
         }

         self.overflow_menu.end(ui);
      }
   }

   fn process_peer_message(&mut self, ui: &mut Ui, message: peer::Message) -> netcanv::Result<()> {
      use peer::MessageKind;

      match message.kind {
         MessageKind::Joined(nickname, peer_id) => {
            log!(
               self.log,
               "{}",
               self
                  .assets
                  .tr
                  .someone_joined_the_room
                  .format()
                  .with("nickname", nickname.as_str())
                  .done()
            );
            if self.peer.is_host() {
               let positions = self.paint_canvas.chunk_positions();
               self.peer.send_chunk_positions(peer_id, positions)?;
            }
            // Order matters here! The tool selection packet must arrive before the packets sent
            // from the tool's `network_peer_join` event.
            self
               .peer
               .send_select_tool(self.toolbar.clone_tool_name(self.toolbar.current_tool()))?;
            self.toolbar.with_current_tool(|tool| {
               tool.network_peer_join(ui, Net::new(&self.peer), peer_id)
            })?;
         }
         MessageKind::Left {
            peer_id,
            nickname,
            last_tool,
         } => {
            log!(
               self.log,
               "{}",
               self
                  .assets
                  .tr
                  .someone_left_the_room
                  .format()
                  .with("nickname", nickname.as_str())
                  .done()
            );
            // Make sure the tool they were last using is properly deinitialized.
            if let Some(tool) = last_tool {
               if let Some(tool_id) = self.toolbar.tool_by_name(&tool) {
                  self.toolbar.with_tool(tool_id, |tool| {
                     tool.network_peer_deactivate(
                        ui,
                        Net::new(&self.peer),
                        &mut self.paint_canvas,
                        peer_id,
                     )
                  })?
               }
            }
         }
         MessageKind::NewHost(nickname) => log!(
            self.log,
            "{}",
            self
               .assets
               .tr
               .someone_is_now_hosting_the_room
               .format()
               .with("nickname", nickname.as_str())
               .done()
         ),
         MessageKind::NowHosting => {
            log!(self.log, "{}", self.assets.tr.you_are_now_hosting_the_room);
            self.chunk_downloads.clear();
         }
         MessageKind::ChunkPositions(positions) => {
            tracing::debug!("received {} chunk positions", positions.len());
            for chunk_position in positions {
               self.chunk_downloads.insert(chunk_position, ChunkDownload::NotDownloaded);
            }
            // Make sure we send the tool _after_ adding the requested chunks.
            // This way if something goes wrong here and the function returns Err, at least we
            // will have queued up some chunk downloads at this point.
            self
               .peer
               .send_select_tool(self.toolbar.clone_tool_name(self.toolbar.current_tool()))?;
         }
         MessageKind::Chunks(chunks) => {
            tracing::debug!("received {} chunks", chunks.len());
            for (chunk_position, image_data) in chunks {
               self.decode_canvas_data(chunk_position, image_data);
               self.chunk_downloads.insert(chunk_position, ChunkDownload::Downloaded);
            }
         }
         MessageKind::GetChunks(requester, positions) => {
            self.encode_chunks(ui, requester, &positions);
         }
         MessageKind::Tool(sender, name, payload) => {
            if let Some(tool_id) = self.toolbar.tool_by_name(&name) {
               self.toolbar.with_tool(tool_id, |tool| {
                  tool.network_receive(
                     ui,
                     Net::new(&self.peer),
                     &mut self.paint_canvas,
                     sender,
                     payload.clone(),
                  )
               })?;
            }
         }
         MessageKind::SelectTool {
            peer_id: address,
            previous_tool,
            tool,
         } => {
            tracing::debug!("{:?} selected tool {}", address, tool);
            // Deselect the old tool.
            if let Some(tool) = previous_tool {
               if let Some(tool_id) = self.toolbar.tool_by_name(&tool) {
                  // ↑ still waiting for if_let_chains to get stabilized.
                  self.toolbar.with_tool(tool_id, |tool| {
                     tool.network_peer_deactivate(
                        ui,
                        Net::new(&self.peer),
                        &mut self.paint_canvas,
                        address,
                     )
                  })?;
               }
            }
            // Select the new tool.
            if let Some(tool_id) = self.toolbar.tool_by_name(&tool) {
               tracing::debug!(" - valid tool - {:?}", tool_id);
               self.toolbar.with_tool(tool_id, |tool| {
                  tool.network_peer_activate(Net::new(&self.peer), address)
               })?;
            }
         }
      }
      Ok(())
   }

   fn encode_chunks(
      &mut self,
      renderer: &mut Backend,
      requester: PeerId,
      positions: &[(i32, i32)],
   ) {
      let tx = &self
         .encoded_chunks
         .entry(requester)
         .or_insert_with(|| {
            let (tx, rx) = mpsc::unbounded_channel();
            EncodeChannels { tx, rx }
         })
         .tx;
      for &chunk_position in positions {
         tracing::info!(
            "fetching data for networking transmission of chunk {:?}",
            chunk_position
         );
         // If there is a cached image already, there's no point in encoding it all over again.
         if let Some(chunk) = self.cache_layer.chunk(chunk_position) {
            tracing::debug!("reusing {:?}", chunk_position);
            let _ = self.encode_channels.tx.send((chunk_position, chunk.to_owned()));
            let _ = tx.send((chunk_position, chunk.to_owned()));
         } else if let Some(chunk) = self.paint_canvas.chunk(chunk_position) {
            // If the chunk's image is empty, there's no point in sending it.
            let image = chunk.download_image(renderer);
            if Chunk::image_is_empty(&image) {
               continue;
            }
            // Otherwise, we can start encoding the chunk image.
            let encoded_chunks_tx = self.encode_channels.tx.clone();
            let tx = tx.clone();

            tokio::spawn(async move {
               tracing::debug!("encoding image data for chunk {:?}", chunk_position);
               let image_data = ImageCoder::encode_network_data(image).await;
               tracing::debug!("encoding done for chunk {:?}", chunk_position);
               match image_data {
                  Ok(data) => {
                     tracing::debug!("sending image data back to main thread");
                     let _ = encoded_chunks_tx.send((chunk_position, data.clone()));
                     let _ = tx.send((chunk_position, data));
                  }
                  Err(error) => {
                     tracing::error!(
                        "error while encoding image for chunk {:?}: {:?}",
                        chunk_position,
                        error
                     );
                  }
               }
            });
         }
      }
   }

   fn reflow_layout(&mut self, root_view: &View) {
      // The bottom bar and the canvas.
      view::layout::vertical(
         root_view,
         &mut [&mut self.bottom_bar_view, &mut self.canvas_view],
         DirectionV::BottomToTop,
      );
      let padded_canvas = view::layout::padded(&self.canvas_view, Self::CANVAS_INNER_PADDING);

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
            assets: &self.assets,
            paint_canvas: &mut self.paint_canvas,
            project_file: &mut self.project_file,
            renderer: ui,
         }) {
            Ok(()) => (),
            Err(error) => log!(
               self.log,
               "{}",
               self
                  .assets
                  .tr
                  .error_while_processing_action
                  .format()
                  .with("error", error.translate(&self.assets.language))
                  .done()
            ),
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
      if !needed_chunks.is_empty() {
         for &chunk_position in &needed_chunks {
            self.chunk_downloads.insert(chunk_position, ChunkDownload::Requested);
         }
         catch!(self.peer.download_chunks(needed_chunks));
      }

      // Error checking

      for message in &bus::retrieve_all::<Error>() {
         let Error(error) = message.consume();
         log!(
            self.log,
            "{}",
            self
               .assets
               .tr
               .error
               .format()
               .with("error", error.translate(&self.assets.language).as_ref())
               .done()
         );
      }
      for _ in &bus::retrieve_all::<Fatal>() {
         self.fatal_error = true;
      }

      // Layout
      self.reflow_layout(&root_view);

      // Paint canvas
      self.process_canvas(ui, input);

      // Bars
      let toolbar_process = self.toolbar.process(
         ui,
         input,
         ToolbarArgs {
            wm: &mut self.wm,
            parent_view: &view::layout::padded(&self.canvas_view, 8.0),
            colors: &self.assets.colors.toolbar,
         },
      );
      if let Some((previous_tool, current_tool)) = toolbar_process.switched {
         self.tool_switch_events(ui.render(), previous_tool, current_tool);
      }
      // Draw windows over the toolbar, but below the bottom bar.
      self.wm.process(ui, input, &self.assets);
      self.process_bar(ui, input);
      self.process_overflow_menu(ui, input);
   }

   fn next_state(self: Box<Self>, _renderer: &mut Backend) -> Box<dyn AppState> {
      if self.fatal_error {
         Box::new(lobby::State::new(self.assets, self.socket_system))
      } else {
         self
      }
   }

   fn exit(self: Box<Self>) {}
}
