//! The paint state. This is the screen where you paint on the canvas with other people.

mod tools;

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use native_dialog::FileDialog;
use netcanv_renderer::paws::{
   point, vector, AlignH, AlignV, Color, Layout, Rect, Renderer, Vector,
};
use netcanv_renderer::{BlendMode, RenderBackend};
use nysa::global as bus;

use crate::app::*;
use crate::assets::*;
use crate::common::*;
use crate::config::UserConfig;
use crate::net::peer::{self, Peer};
use crate::net::timer::Timer;
use crate::paint_canvas::*;
use crate::ui::*;
use crate::viewport::Viewport;

use self::tools::Tool;

/// The current mode of painting.
///
/// This is either `Paint` or `Erase` when the mouse buttons are held, and `None` when it's
/// released.
#[derive(PartialEq, Eq)]
enum PaintMode {
   None,
   Paint,
   Erase,
}

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
   const BAR_SIZE: f32 = 32.0;
   /// The network communication tick interval.
   pub const TIME_PER_UPDATE: Duration = Duration::from_millis(50);

   /// Creates a new paint state.
   pub fn new(assets: Assets, config: UserConfig, peer: Peer, image_path: Option<PathBuf>) -> Self {
      let mut this = Self {
         assets,
         config,

         paint_canvas: PaintCanvas::new(),
         tools: Rc::new(RefCell::new(Vec::new())),
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
      this.register_tools();

      if this.peer.is_host() {
         log!(this.log, "Welcome to your room!");
         log!(
            this.log,
            "To invite friends, send them the room ID shown in the bottom right corner of your screen."
         );
      }

      this
   }

   /// Registers all the tools.
   fn register_tools(&mut self) {
      let mut tools = self.tools.borrow_mut();
      tools.push(Box::new(tools::Brush::new()));
   }

   /// Executes the given callback with the currently selected tool.
   fn with_current_tool(&mut self, mut callback: impl FnMut(&mut Self, &mut Box<dyn Tool>)) {
      let tools = Rc::clone(&self.tools);
      let mut tools = tools.borrow_mut();
      let tool = &mut tools[self.current_tool];
      callback(self, tool);
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

   /// Performs a fellow peer's stroke on the canvas.
   fn fellow_stroke(&mut self, ui: &mut Ui, points: &[StrokePoint]) {
      if points.is_empty() {
         return;
      } // failsafe

      let mut from = points[0].point;
      let first_index = if points.len() > 1 { 1 } else { 0 };
      for point in &points[first_index..] {
         self.paint_canvas.stroke(ui.render(), from, point.point, &point.brush);
         from = point.point;
      }
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

   /// Processes the paint canvas.
   fn process_canvas(&mut self, ui: &mut Ui, input: &Input) {
      ui.push((ui.width(), ui.height() - Self::BAR_SIZE), Layout::Freeform);
      let canvas_size = ui.size();

      //
      // Input
      //

      // Drawing

      self.with_current_tool(|p, tool| {
         tool.process_paint_canvas_input(ui, input, &mut p.paint_canvas, &p.viewport)
      });

      // Panning and zooming

      if ui.has_mouse(input) && input.mouse_button_just_pressed(MouseButton::Middle) {
         self.panning = true;
      }
      if input.mouse_button_just_released(MouseButton::Middle) {
         self.panning = false;
      }

      if self.panning {
         let delta_pan = input.previous_mouse_position() - input.mouse_position();
         self.viewport.pan_around(delta_pan);
         let pan = self.viewport.pan();
         let position = format!("{}, {}", (pan.x / 256.0).floor(), (pan.y / 256.0).floor());
         self.show_tip(&position, Duration::from_millis(100));
      }
      if input.mouse_scroll().y != 0.0 {
         self.viewport.zoom_in(input.mouse_scroll().y);
         self.show_tip(
            &format!("{:.0}%", self.viewport.zoom() * 100.0),
            Duration::from_secs(3),
         );
      }

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

         let color = Color::WHITE.with_alpha(240);

         ui.render().push();
         ui.render().set_blend_mode(BlendMode::Invert);

         for (_, mate) in self.peer.mates() {
            let cursor = self.viewport.to_screen_space(mate.lerp_cursor(), canvas_size);
            let brush_radius = mate.brush_size * self.viewport.zoom() * 0.5;
            let text_position = cursor + point(brush_radius, brush_radius);
            ui.render().text(
               Rect::new(text_position, vector(0.0, 0.0)),
               &self.assets.sans,
               &mate.nickname,
               color,
               (AlignH::Left, AlignV::Top),
            );
            ui.render().outline_circle(cursor, brush_radius, color, 1.0);
         }

         self.with_current_tool(|p, tool| {
            tool.process_paint_canvas_overlays(ui, input, &p.viewport);
         });

         ui.render().pop();
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

      for _ in self.update_timer.tick() {
         // Mouse / drawing
         // if input.previous_mouse_position() != input.mouse_position() {
         //    catch!(self.peer.send_cursor(to, brush_size));
         // }
         // if !self.stroke_buffer.is_empty() {
         //    catch!(self.peer.send_stroke(self.stroke_buffer.drain(..)));
         // }
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
               if let Some(ChunkDownload::NotDownloaded) = self.chunk_downloads.get(&chunk_position)
               {
                  Self::queue_chunk_download(chunk_position);
               }
            }
         }
      }
   }

   /// Processes the bottom bar.
   fn process_bar(&mut self, ui: &mut Ui, input: &mut Input) {
      // if self.paint_mode != PaintMode::None {
      //    input.lock_mouse_buttons();
      // }

      ui.push((ui.width(), ui.remaining_height()), Layout::Horizontal);
      ui.fill(self.assets.colors.panel);
      ui.pad((8.0, 0.0));

      // Color palette

      // for &color in COLOR_PALETTE {
      //    ui.push((16.0, ui.height()), Layout::Freeform);
      //    let y_offset = ui.height()
      //       * if self.paint_color == color {
      //          0.5
      //       } else if ui.has_mouse(&input) {
      //          0.7
      //       } else {
      //          0.8
      //       };
      //    let y_offset = y_offset.round();
      //    if ui.has_mouse(&input) && input.mouse_button_just_pressed(MouseButton::Left) {
      //       self.paint_color = color.clone();
      //    }
      //    ui.draw(|ui| {
      //       let rect = Rect::new(point(0.0, y_offset), ui.size());
      //       ui.render().fill(rect, color, 4.0);
      //    });
      //    ui.pop();
      // }
      // ui.space(16.0);

      // // Brush size

      // ui.push((80.0, ui.height()), Layout::Freeform);
      // ui.text(
      //    &self.assets.sans,
      //    "Brush size",
      //    self.assets.colors.text,
      //    (AlignH::Center, AlignV::Middle),
      // );
      // ui.pop();

      // ui.space(8.0);
      // self.brush_size_slider.process(
      //    ui,
      //    input,
      //    SliderArgs {
      //       width: 192.0,
      //       color: self.assets.colors.slider,
      //    },
      // );
      // ui.space(8.0);

      // let brush_size_string = self.brush_size_slider.value().to_string();
      // ui.push((ui.height(), ui.height()), Layout::Freeform);
      // ui.text(
      //    &self.assets.sans,
      //    &brush_size_string,
      //    self.assets.colors.text,
      //    (AlignH::Center, AlignV::Middle),
      // );
      // ui.pop();

      //
      // Right side
      //

      // Room ID display

      // Note that elements in HorizontalRev go from right to left rather than left to right.
      ui.push((ui.remaining_width(), ui.height()), Layout::HorizontalRev);
      if Button::with_icon(
         ui,
         input,
         ButtonArgs {
            font: &self.assets.sans,
            height: 32.0,
            colors: &self.assets.colors.tool_button,
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
      if self.peer.is_host() {
         // The room ID itself
         let id_text = format!("{:04}", self.peer.room_id().unwrap());
         ui.push((64.0, ui.height()), Layout::Freeform);
         ui.text(
            &self.assets.sans_bold,
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
      }
      ui.pop();

      ui.pop();

      input.unlock_mouse_buttons();
   }

   fn process_peer_message(&mut self, ui: &mut Ui, message: peer::Message) -> anyhow::Result<()> {
      use peer::MessageKind;

      match message.kind {
         MessageKind::Joined(nickname, address) => {
            log!(self.log, "{} joined the room", nickname);
            if self.peer.is_host() {
               let positions = self.paint_canvas.chunk_positions();
               self.peer.send_chunk_positions(address, positions)?;
            }
         }
         MessageKind::Left(nickname) => {
            log!(self.log, "{} has left", nickname);
         }
         MessageKind::Stroke(points) => {
            self.fellow_stroke(ui, &points);
         }
         MessageKind::ChunkPositions(positions) => {
            eprintln!("received {} chunk positions", positions.len());
            for chunk_position in positions {
               self.chunk_downloads.insert(chunk_position, ChunkDownload::NotDownloaded);
            }
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
      }
      Ok(())
   }

   fn send_chunks(&mut self, address: SocketAddr, positions: &[(i32, i32)]) -> anyhow::Result<()> {
      const KILOBYTE: usize = 1024;
      const MAX_BYTES_PER_PACKET: usize = 128 * KILOBYTE;

      let mut packet = Vec::new();
      let mut bytes_of_image_data = 0;
      for &chunk_position in positions {
         if bytes_of_image_data > MAX_BYTES_PER_PACKET {
            let packet = std::mem::replace(&mut packet, Vec::new());
            bytes_of_image_data = 0;
            self.peer.send_chunks(address, packet)?;
         }
         if let Some(image_data) = self.paint_canvas.network_data(chunk_position) {
            packet.push((chunk_position, image_data.to_owned()));
            bytes_of_image_data += image_data.len();
         }
      }
      self.peer.send_chunks(address, packet)?;

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

      // Bar
      self.process_bar(ui, input);
   }

   fn next_state(self: Box<Self>) -> Box<dyn AppState> {
      if self.fatal_error {
         Box::new(lobby::State::new(self.assets, self.config))
      } else {
         self
      }
   }
}
