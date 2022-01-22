// The lobby app state.

use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

use native_dialog::FileDialog;
use netcanv_protocol::relay::{self, RoomId};
use netcanv_renderer::paws::{vector, AlignH, AlignV, Layout};
use netcanv_renderer::{Font, RenderBackend};
use nysa::global as bus;

use crate::app::{paint, AppState, StateArgs};
use crate::assets::{self, Assets, ColorScheme};
use crate::backend::Backend;
use crate::common::{Error, Fatal};
use crate::config::{self, config};
use crate::net::peer::{self, Peer};
use crate::net::socket::SocketSystem;
use crate::ui::*;

/// A status returned from some other part of the app.
#[derive(Debug)]
enum Status {
   None,
   Info(String),
   Error(String),
}

impl<T: Display> From<T> for Status {
   fn from(error: T) -> Self {
      Self::Error(format!("{}", error))
   }
}

/// The lobby app state.
pub struct State {
   assets: Assets,

   // Subsystems
   relay_socksys: Arc<SocketSystem<relay::Packet>>,

   // UI elements
   nickname_field: TextField,
   relay_field: TextField,
   room_id_field: TextField,

   join_expand: Expand,
   host_expand: Expand,

   // net
   status: Status,
   peer: Option<Peer>,
   image_file: Option<PathBuf>, // when this is Some, the canvas is loaded from a file
}

impl State {
   /// Creates and initializes the lobby state.
   pub fn new(assets: Assets) -> Self {
      let nickname_field = TextField::new(Some(&config().lobby.nickname));
      let relay_field = TextField::new(Some(&config().lobby.relay));
      Self {
         assets,

         relay_socksys: SocketSystem::new(),

         nickname_field,
         relay_field,
         room_id_field: TextField::new(None),

         join_expand: Expand::new(true),
         host_expand: Expand::new(false),

         status: Status::None,
         peer: None,
         image_file: None,
      }
   }

   /// Processes the header (app name and welcome message).
   fn process_header(&mut self, ui: &mut Ui) {
      ui.push((ui.width(), 72.0), Layout::Vertical);

      ui.push((ui.width(), 56.0), Layout::Freeform);
      ui.text(
         &self.assets.sans.with_size(48.0),
         "NetCanv",
         self.assets.colors.text,
         (AlignH::Left, AlignV::Middle),
      );
      ui.pop();

      ui.push((ui.width(), ui.remaining_height()), Layout::Freeform);
      ui.text(
         &self.assets.sans,
         "Welcome! Host a room or join an existing one to start painting.",
         self.assets.colors.text,
         (AlignH::Left, AlignV::Middle),
      );
      ui.pop();

      ui.pop();
   }

   /// Processes the connection menu (nickname and relay fields and two Expands with options
   /// for joining or hosting a room).
   fn process_menu(&mut self, ui: &mut Ui, input: &mut Input) -> Option<Box<dyn AppState>> {
      ui.push((ui.width(), ui.remaining_height()), Layout::Vertical);

      let button = ButtonArgs {
         height: 32.0,
         colors: &self.assets.colors.button.clone(),
         corner_radius: 0.0,
      };
      let textfield = TextFieldArgs {
         font: &self.assets.sans,
         width: 160.0,
         colors: &self.assets.colors.text_field,
         hint: None,
      };
      let expand = ExpandArgs {
         font: &self.assets.sans.with_size(22.0),
         label: "",
         icons: &self.assets.icons.expand,
         colors: &self.assets.colors.expand,
      };

      // nickname, relay
      ui.push(
         (ui.width(), TextField::labelled_height(textfield.font)),
         Layout::Horizontal,
      );
      self.nickname_field.with_label(
         ui,
         input,
         &self.assets.sans,
         "Nickname",
         TextFieldArgs {
            hint: Some("Name shown to others"),
            ..textfield
         },
      );
      ui.space(16.0);
      self.relay_field.with_label(
         ui,
         input,
         &self.assets.sans,
         "Relay server",
         TextFieldArgs {
            hint: Some("IP address"),
            ..textfield
         },
      );
      ui.pop();
      ui.space(32.0);

      // join room
      if self
         .join_expand
         .process(
            ui,
            input,
            ExpandArgs {
               label: "Join an existing room",
               ..expand
            },
         )
         .mutually_exclude(&mut self.host_expand)
         .expanded()
      {
         ui.push(ui.remaining_size(), Layout::Vertical);
         ui.offset(vector(32.0, 8.0));

         ui.paragraph(
            &self.assets.sans,
            &[
               "Ask your friend for the Room ID",
               "and enter it into the text field below.",
            ],
            self.assets.colors.text,
            AlignH::Left,
            None,
         );
         ui.space(16.0);
         ui.push(
            (0.0, TextField::labelled_height(textfield.font)),
            Layout::Horizontal,
         );
         let room_id_field = self.room_id_field.with_label(
            ui,
            input,
            &self.assets.sans,
            "Room ID",
            TextFieldArgs {
               hint: Some("6 characters"),
               font: &self.assets.monospace,
               ..textfield
            },
         );
         ui.offset(vector(16.0, 16.0));
         if Button::with_text(ui, input, button, &self.assets.sans, "Join").clicked()
            || room_id_field.done()
         {
            match Self::join_room(
               &self.relay_socksys,
               self.nickname_field.text(),
               self.relay_field.text(),
               self.room_id_field.text(),
            ) {
               Ok(peer) => {
                  self.peer = Some(peer);
                  self.status = Status::Info("Connecting…".into());
               }
               Err(status) => self.status = status,
            }
         }
         ui.pop();

         ui.fit();
         ui.pop();
      }
      ui.space(16.0);

      // host room
      if self
         .host_expand
         .process(
            ui,
            input,
            ExpandArgs {
               label: "Host a new room",
               ..expand
            },
         )
         .mutually_exclude(&mut self.join_expand)
         .expanded()
      {
         ui.push(ui.remaining_size(), Layout::Vertical);
         ui.offset(vector(32.0, 8.0));

         ui.paragraph(
            &self.assets.sans,
            &[
               "Create a blank canvas, or load an existing one from file,",
               "and share the Room ID with your friends.",
            ],
            self.assets.colors.text,
            AlignH::Left,
            None,
         );
         ui.space(16.0);

         macro_rules! host_room {
            () => {
               self.status = Status::Info("Connecting…".into());
               match Self::host_room(
                  &self.relay_socksys,
                  self.nickname_field.text(),
                  self.relay_field.text(),
               ) {
                  Ok(peer) => self.peer = Some(peer),
                  Err(status) => self.status = status,
               }
            };
         }

         ui.push((ui.remaining_width(), 32.0), Layout::Horizontal);
         if Button::with_text(ui, input, button, &self.assets.sans, "Host").clicked() {
            host_room!();
         }
         ui.space(8.0);
         if Button::with_text(ui, input, button, &self.assets.sans, "from File").clicked() {
            match FileDialog::new()
               .set_filename("canvas.png")
               .add_filter("Supported image files", &["png", "jpg", "jpeg", "jfif"])
               .add_filter("NetCanv canvas", &["toml"])
               .show_open_single_file()
            {
               Ok(Some(path)) => {
                  self.image_file = Some(path);
                  host_room!();
               }
               Err(error) => self.status = Status::from(error),
               _ => (),
            }
         }
         ui.pop();

         ui.fit();
         ui.pop();
      }

      ui.pop();

      chain_focus(
         input,
         &mut [
            &mut self.nickname_field,
            &mut self.relay_field,
            &mut self.room_id_field,
         ],
      );

      None
   }

   /// Processes the status report box.
   fn process_status(&mut self, ui: &mut Ui) {
      if !matches!(self.status, Status::None) {
         ui.push((ui.width(), 24.0), Layout::Horizontal);
         let icon = match self.status {
            Status::None => unreachable!(),
            Status::Info(_) => &self.assets.icons.status.info,
            Status::Error(_) => &self.assets.icons.status.error,
         };
         let color = match self.status {
            Status::None => unreachable!(),
            Status::Info(_) => self.assets.colors.text,
            Status::Error(_) => self.assets.colors.error,
         };
         ui.icon(icon, color, Some(vector(ui.height(), ui.height())));
         ui.space(8.0);
         ui.push((ui.remaining_width(), ui.height()), Layout::Freeform);
         let text = match &self.status {
            Status::None => unreachable!(),
            Status::Info(text) | Status::Error(text) => text,
         };
         ui.text(
            &self.assets.sans,
            text,
            color,
            (AlignH::Left, AlignV::Middle),
         );
         ui.pop();
         ui.pop();
      }
   }

   /// Checks whether a nickname is valid.
   fn validate_nickname(nickname: &str) -> Result<(), Status> {
      if nickname.is_empty() {
         return Err(Status::Error("Nickname must not be empty".into()));
      }
      if nickname.len() > 16 {
         return Err(Status::Error(
            "The maximum length of a nickname is 16 characters".into(),
         ));
      }
      Ok(())
   }

   /// Establishes a connection to the relay and hosts a new room.
   fn host_room(
      socksys: &Arc<SocketSystem<relay::Packet>>,
      nickname: &str,
      relay_addr_str: &str,
   ) -> Result<Peer, Status> {
      Self::validate_nickname(nickname)?;
      Ok(Peer::host(socksys, nickname, relay_addr_str)?)
   }

   /// Establishes a connection to the relay and joins an existing room.
   fn join_room(
      socksys: &Arc<SocketSystem<relay::Packet>>,
      nickname: &str,
      relay_addr_str: &str,
      room_id_str: &str,
   ) -> Result<Peer, Status> {
      if room_id_str.len() != 6 {
         return Err(Status::Error(
            "Room ID must be a code with 6 characters".into(),
         ));
      }
      Self::validate_nickname(nickname)?;
      let room_id = RoomId::try_from(room_id_str)?;
      Ok(Peer::join(socksys, nickname, relay_addr_str, room_id)?)
   }

   /// Saves the user configuration.
   fn save_config(&mut self) {
      config::write(|config| {
         config.lobby.nickname = self.nickname_field.text().to_owned();
         config.lobby.relay = self.relay_field.text().to_owned();
      });
   }
}

impl AppState for State {
   fn process(&mut self, StateArgs { ui, input, .. }: StateArgs) {
      ui.clear(self.assets.colors.panel);

      // The lobby does not use mouse areas.
      input.set_mouse_area(0, true);

      if let Some(peer) = &mut self.peer {
         catch!(peer.communicate());
      }

      ui.pad((32.0, 32.0));

      ui.push((ui.width(), 384.0), Layout::Vertical);
      ui.align((AlignH::Left, AlignV::Middle));
      self.process_header(ui);
      ui.space(24.0);
      self.process_menu(ui, input);
      ui.space(24.0);
      self.process_status(ui);
      ui.pop();

      ui.push((32.0, ui.height()), Layout::Vertical);
      ui.align((AlignH::Right, AlignV::Top));

      if Button::with_icon(
         ui,
         input,
         ButtonArgs {
            height: 32.0,
            colors: &self.assets.colors.action_button,
            corner_radius: 4.0,
         },
         if config().ui.color_scheme == config::ColorScheme::Dark {
            &self.assets.icons.lobby.light_mode
         } else {
            &self.assets.icons.lobby.dark_mode
         },
      )
      .clicked()
      {
         config::write(|config| {
            config.ui.color_scheme = match config.ui.color_scheme {
               config::ColorScheme::Light => config::ColorScheme::Dark,
               config::ColorScheme::Dark => config::ColorScheme::Light,
            };
         });
         self.save_config();
         self.assets.colors = ColorScheme::from(config().ui.color_scheme);
      }

      if assets::has_license_page() {
         ui.push((ui.width(), ui.remaining_height()), Layout::VerticalRev);
         if Button::with_icon(
            ui,
            input,
            ButtonArgs {
               height: 32.0,
               colors: &self.assets.colors.action_button,
               corner_radius: 4.0,
            },
            &self.assets.icons.lobby.legal,
         )
         .clicked()
         {
            catch!(assets::open_license_page());
         }
         ui.pop();
      }

      ui.pop();

      for message in &bus::retrieve_all::<Error>() {
         let error = message.consume().0;
         eprintln!("error: {}", error);
         self.status = Status::Error(error.to_string());
      }
      for message in &bus::retrieve_all::<Fatal>() {
         let fatal = message.consume().0;
         eprintln!("fatal: {}", fatal);
         self.status = Status::Error(format!("Fatal: {}", fatal));
      }
   }

   fn next_state(self: Box<Self>, renderer: &mut Backend) -> Box<dyn AppState> {
      let mut connected = false;
      if let Some(peer) = &self.peer {
         for message in &bus::retrieve_all::<peer::Connected>() {
            eprintln!("connection established");
            if message.peer == peer.token() {
               message.consume();
               connected = true;
            }
         }
      }

      if connected {
         let mut this = *self;
         this.save_config();
         match paint::State::new(this.assets, this.peer.unwrap(), this.image_file, renderer) {
            Ok(state) => Box::new(state),
            Err((error, assets)) => {
               bus::push(Fatal(error));
               Box::new(Self::new(assets))
            }
         }
      } else {
         self
      }
   }
}
