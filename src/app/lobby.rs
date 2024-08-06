// The lobby app state.

use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

use native_dialog::FileDialog;
use netcanv_i18n::translate_enum::TranslateEnum;
use netcanv_protocol::relay::RoomId;
use netcanv_renderer::paws::{vector, AlignH, AlignV, Color, Layout, LineCap, Rect, Renderer};
use netcanv_renderer::{Font, Image as ImageTrait, RenderBackend};
use nysa::global as bus;

use crate::app::{paint, AppState, StateArgs};
use crate::assets::{self, Assets, ColorScheme};
use crate::backend::Backend;
use crate::common::{Error, Fatal, StrExt};
use crate::config::{self, config};
use crate::net::peer::{self, Peer};
use crate::net::socket::SocketSystem;
use crate::strings::Strings;
use crate::ui::view::View;
use crate::ui::*;

/// Colors used in the lobby screen.
#[derive(Clone)]
pub struct LobbyColors {
   pub background: Color,
}

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
   assets: Box<Assets>,

   // Subsystems
   socket_system: Arc<SocketSystem>,

   // UI elements
   nickname_field: TextField,
   relay_field: TextField,
   room_id_field: TextField,

   join_expand: Expand,
   host_expand: Expand,

   main_view: View,
   panel_view: View,
   language_menu: ContextMenu,

   // net
   status: Status,
   peer: Option<Peer>,
   image_file: Option<PathBuf>, // when this is Some, the canvas is loaded from a file
}

impl State {
   const BANNER_HEIGHT: f32 = 128.0;
   const MENU_HEIGHT: f32 = 294.0;
   const STATUS_HEIGHT: f32 = 8.0 + 48.0;

   const VIEW_BOX_PADDING: f32 = 16.0;
   const VIEW_BOX_WIDTH: f32 = 388.0 + Self::VIEW_BOX_PADDING * 2.0;
   const VIEW_BOX_HEIGHT: f32 = Self::MENU_HEIGHT + Self::VIEW_BOX_PADDING * 2.0;

   /// Creates and initializes the lobby state.
   pub fn new(assets: Box<Assets>, socket_system: Arc<SocketSystem>) -> Self {
      let nickname_field = TextField::new(Some(&config().lobby.nickname));
      let relay_field = TextField::new(Some(&config().lobby.relay));
      let mut this = Self {
         socket_system,

         nickname_field,
         relay_field,
         room_id_field: TextField::new(None),

         join_expand: Expand::new(true),
         host_expand: Expand::new(false),

         main_view: View::new((
            Self::VIEW_BOX_WIDTH,
            Self::BANNER_HEIGHT + Self::VIEW_BOX_HEIGHT + Self::STATUS_HEIGHT,
         )),
         panel_view: View::new((40.0, 4.0 + 3.0 * 36.0)),
         // The size of the language menu is computed later.
         language_menu: ContextMenu::new((0.0, 0.0)),

         assets,

         status: Status::None,
         peer: None,
         image_file: None,
      };
      this.room_id_field.set_focus(true);
      this
   }

   /// Processes the logo banner.
   fn process_banner(&mut self, ui: &mut Ui, input: &Input, root_view: &View) {
      ui.push((ui.width(), Self::BANNER_HEIGHT), Layout::Freeform);

      let group_rect = ui.rect();
      let scale = group_rect.height() / self.assets.banner.base.height() as f32;
      let image_size = vector(
         self.assets.banner.base.width() as f32,
         self.assets.banner.base.height() as f32,
      ) * scale;
      let image_rect = Rect::new(group_rect.center() - image_size / 2.0, image_size);

      ui.image(image_rect, &self.assets.banner.shadow);

      const STRIP_X_POSITIONS: [f32; 3] = [8.0, 48.0, 88.0];
      const STRIP_COLORS: [Color; 3] = [
         Color::rgb(0xFF003E),
         Color::rgb(0x2DD70E),
         Color::rgb(0x0868EB),
      ];
      const STRIP_WIDTH: f32 = 16.0;
      let strip_width = STRIP_WIDTH * scale;

      const SUBDIVISION_SPACING: f32 = 12.0;
      let main_view_rect = self.main_view.rect();
      let flat_range = (image_rect.top() + image_rect.height() * 0.5)
         ..(main_view_rect.bottom() - Self::STATUS_HEIGHT);
      let flat_radius = (flat_range.end - flat_range.start) / 2.0;
      let waving_center = (flat_range.start + flat_range.end) / 2.0;
      let subdivisions = (root_view.height() / SUBDIVISION_SPACING).ceil() as usize;

      for (&x, &color) in STRIP_X_POSITIONS.iter().zip(STRIP_COLORS.iter()) {
         const AMPLITUDE_SCALE: f32 = 0.1;
         const FREQUENCY: f32 = 0.03 * std::f32::consts::PI;
         const MAX_AMPLITUDE: f32 = 64.0;
         let mut previous_coords = None;
         for i in 0..=subdivisions {
            let y = SUBDIVISION_SPACING * i as f32;
            let point_amplitude = (f32::abs(y - waving_center) - flat_radius)
               .clamp(0.0, MAX_AMPLITUDE)
               * AMPLITUDE_SCALE;
            let wave = f32::sin(-((y - waving_center) * FREQUENCY).abs() + input.time_in_seconds())
               * point_amplitude;
            let x = (x + wave) * scale;
            let x = image_rect.x() + x;
            let point = vector(x + strip_width / 2.0, y);
            if let Some(previous_point) = previous_coords {
               ui.render().line(previous_point, point, color, LineCap::Round, strip_width);
            }
            previous_coords = Some(point);
         }
      }

      ui.image(image_rect, &self.assets.banner.base);

      ui.pop();
   }

   /// Processes the welcome message.
   fn process_welcome(&mut self, ui: &mut Ui) {
      ui.push((ui.width(), 24.0), Layout::Vertical);

      ui.push((ui.width(), ui.remaining_height()), Layout::Freeform);
      ui.text(
         &self.assets.sans,
         &self.assets.tr.lobby_welcome,
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

      let button = ButtonArgs::new(ui, &self.assets.colors.button).height(32.0).pill();
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
         &self.assets.tr.lobby_nickname.label,
         TextFieldArgs {
            hint: Some(&self.assets.tr.lobby_nickname.hint),
            ..textfield
         },
      );
      ui.space(16.0);
      self.relay_field.with_label(
         ui,
         input,
         &self.assets.sans,
         &self.assets.tr.lobby_relay_server.label,
         TextFieldArgs {
            hint: Some(&self.assets.tr.lobby_relay_server.hint),
            ..textfield
         },
      );
      ui.pop();
      ui.space(24.0);

      // join room
      if self
         .join_expand
         .process(
            ui,
            input,
            ExpandArgs {
               label: &self.assets.tr.lobby_join_a_room.title,
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
            self.assets.tr.lobby_join_a_room.description.split('\n'),
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
            &self.assets.tr.lobby_room_id.label,
            TextFieldArgs {
               hint: Some(&self.assets.tr.lobby_room_id.hint),
               font: &self.assets.monospace,
               ..textfield
            },
         );
         ui.offset(vector(8.0, 16.0));
         if Button::with_text(
            ui,
            input,
            &button,
            &self.assets.sans,
            &self.assets.tr.lobby_join,
         )
         .clicked()
            || room_id_field.done()
         {
            match Self::join_room(
               Arc::clone(&self.socket_system),
               &self.assets.tr,
               self.nickname_field.text().strip_whitespace(),
               self.relay_field.text().strip_whitespace(),
               self.room_id_field.text().strip_whitespace(),
            ) {
               Ok(peer) => {
                  self.peer = Some(peer);
                  self.status = Status::Info(self.assets.tr.connecting.clone());
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
               label: &self.assets.tr.lobby_host_a_new_room.title,
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
            self.assets.tr.lobby_host_a_new_room.description.split('\n'),
            self.assets.colors.text,
            AlignH::Left,
            None,
         );
         ui.space(16.0);

         macro_rules! host_room {
            () => {
               self.status = Status::Info(self.assets.tr.connecting.clone());
               match Self::host_room(
                  Arc::clone(&self.socket_system),
                  &self.assets.tr,
                  self.nickname_field.text().strip_whitespace(),
                  self.relay_field.text().strip_whitespace(),
               ) {
                  Ok(peer) => self.peer = Some(peer),
                  Err(status) => self.status = status,
               }
            };
         }

         ui.push((ui.remaining_width(), 32.0), Layout::Horizontal);
         if Button::with_text(
            ui,
            input,
            &button,
            &self.assets.sans,
            &self.assets.tr.lobby_host,
         )
         .clicked()
         {
            host_room!();
         }
         ui.space(8.0);
         if Button::with_text(
            ui,
            input,
            &button,
            &self.assets.sans,
            &self.assets.tr.lobby_host_from_file,
         )
         .clicked()
         {
            match FileDialog::new()
               .set_filename("canvas.png")
               .add_filter(
                  &self.assets.tr.fd_supported_image_files,
                  &["png", "jpg", "jpeg", "jfif"],
               )
               .add_filter(&self.assets.tr.fd_netcanv_canvas, &["toml"])
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
   fn process_status(&mut self, ui: &mut Ui, input: &mut Input) {
      if !matches!(self.status, Status::None) {
         let (icon, color, text) = match &self.status {
            Status::None => unreachable!(),
            Status::Info(text) => (
               &self.assets.icons.status.info,
               self.assets.colors.text,
               text,
            ),
            Status::Error(text) => (
               &self.assets.icons.status.error,
               self.assets.colors.error,
               text,
            ),
         };
         let width = 56.0 + self.assets.sans.text_width(text);
         let width = width.max(ui.width());
         let width = (width / 2.0).ceil() * 2.0;
         let mut status_view = View::new((width, 48.0));
         view::layout::align(
            &self.main_view,
            &mut status_view,
            (AlignH::Center, AlignV::Bottom),
         );
         status_view.begin(ui, input, Layout::Horizontal);
         ui.fill_rounded(self.assets.colors.panel, 8.0);
         ui.pad(16.0);
         ui.icon(icon, color, Some(vector(ui.height(), ui.height())));
         ui.space(8.0);
         ui.push((ui.remaining_width(), ui.height()), Layout::Freeform);
         ui.text(
            &self.assets.sans,
            text,
            color,
            (AlignH::Left, AlignV::Middle),
         );
         ui.pop();
         status_view.end(ui);
      }
   }

   /// Processes the panel on the right that contains action buttons.
   fn process_icon_panel(&mut self, ui: &mut Ui, input: &mut Input) {
      if Button::with_icon(
         ui,
         input,
         &ButtonArgs::new(ui, &self.assets.colors.action_button).height(32.0).pill().tooltip(
            &self.assets.sans,
            Tooltip::left(match config().ui.color_scheme {
               config::ColorScheme::Light => &self.assets.tr.switch_to_dark_mode,
               config::ColorScheme::Dark => &self.assets.tr.switch_to_light_mode,
            }),
         ),
         match config().ui.color_scheme {
            config::ColorScheme::Dark => &self.assets.icons.lobby.light_mode,
            config::ColorScheme::Light => &self.assets.icons.lobby.dark_mode,
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

      ui.space(4.0);

      let language_button = Button::with_icon(
         ui,
         input,
         &ButtonArgs::new(ui, &self.assets.colors.action_button)
            .height(32.0)
            .pill()
            .tooltip(&self.assets.sans, Tooltip::left(&self.assets.tr.language)),
         &self.assets.icons.lobby.translate,
      );
      let n_languages = self.assets.languages.len() as f32;
      let language_menu_rect = TooltipPosition::Left.compute_rect(
         ui,
         language_button.group(),
         vector(128.0, 16.0 + n_languages * 24.0 + (n_languages - 1.0) * 4.0),
         TooltipLayout {
            spacing: 24.0,
            root_padding: 8.0,
         },
      );
      view::layout::absolute(&mut self.language_menu.view, language_menu_rect);
      if language_button.clicked() {
         self.language_menu.toggle();
      }

      ui.space(4.0);

      if assets::has_license_page()
         && Button::with_icon(
            ui,
            input,
            &ButtonArgs::new(ui, &self.assets.colors.action_button).height(32.0).pill().tooltip(
               &self.assets.sans,
               Tooltip::left(&self.assets.tr.open_source_licenses),
            ),
            &self.assets.icons.lobby.legal,
         )
         .clicked()
      {
         catch!(assets::open_license_page());
      }
   }

   fn process_language_menu(&mut self, ui: &mut Ui, input: &mut Input) {
      if self
         .language_menu
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
         let mut changed = false;
         for (name, code) in self.assets.languages.iter() {
            if Button::with_text_width(
               ui,
               input,
               &ButtonArgs::new(ui, &self.assets.colors.action_button).height(24.0).pill(),
               if code == &config().language {
                  &self.assets.sans_bold
               } else {
                  &self.assets.sans
               },
               name,
               ui.width(),
            )
            .clicked()
            {
               config::write(|config| {
                  config.language.clone_from(code);
               });
               changed = true;
            }
            ui.space(4.0);
         }
         if changed {
            catch!(self.assets.reload_language());
         }
         self.language_menu.end(ui);
      }
   }

   /// Checks whether a nickname is valid.
   fn validate_nickname(tr: &Strings, nickname: &str) -> Result<(), Status> {
      const MAX_LEN: usize = 16;
      if nickname.is_empty() {
         return Err(Status::Error(tr.error_nickname_must_not_be_empty.clone()));
      }
      if nickname.len() > 16 {
         return Err(Status::Error(
            tr.error_nickname_too_long.format().with("max-length", MAX_LEN).done(),
         ));
      }
      Ok(())
   }

   /// Establishes a connection to the relay and hosts a new room.
   fn host_room(
      socket_system: Arc<SocketSystem>,
      tr: &Strings,
      nickname: &str,
      relay_addr_str: &str,
   ) -> Result<Peer, Status> {
      Self::validate_nickname(tr, nickname)?;
      Ok(Peer::host(socket_system, nickname, relay_addr_str))
   }

   /// Establishes a connection to the relay and joins an existing room.
   fn join_room(
      socket_system: Arc<SocketSystem>,
      tr: &Strings,
      nickname: &str,
      relay_addr_str: &str,
      room_id_str: &str,
   ) -> Result<Peer, Status> {
      if room_id_str.len() != RoomId::LEN {
         return Err(Status::Error(
            tr.error_invalid_room_id_length.format().with("length", RoomId::LEN).done(),
         ));
      }
      Self::validate_nickname(tr, nickname)?;
      let room_id = room_id_str.parse()?;
      Ok(Peer::join(socket_system, nickname, relay_addr_str, room_id))
   }

   /// Saves the user configuration.
   fn save_config(&mut self) {
      config::write(|config| {
         self.nickname_field.text().strip_whitespace().clone_into(&mut config.lobby.nickname);
         self.relay_field.text().strip_whitespace().clone_into(&mut config.lobby.relay);
      });
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
      ui.clear(self.assets.colors.lobby.background);

      // The lobby does not use mouse areas.
      input.set_mouse_area(0, true);

      if let Some(peer) = &mut self.peer {
         catch!(peer.communicate());
      }

      let padded_root_view = view::layout::padded(&root_view, 8.0);
      view::layout::align(
         &root_view,
         &mut self.main_view,
         (AlignH::Center, AlignV::Middle),
      );
      view::layout::align(
         &padded_root_view,
         &mut self.panel_view,
         (AlignH::Right, AlignV::Top),
      );

      // Main view

      self.main_view.begin(ui, input, Layout::Vertical);

      self.process_banner(ui, input, &root_view);

      ui.push((ui.width(), Self::VIEW_BOX_HEIGHT), Layout::Vertical);
      ui.fill_rounded(self.assets.colors.panel, 8.0);

      ui.push(ui.size(), Layout::Vertical);
      ui.pad(Self::VIEW_BOX_PADDING);

      self.process_welcome(ui);
      ui.space(24.0);
      self.process_menu(ui, input);

      ui.pop();

      ui.space(40.0);
      self.process_status(ui, input);

      ui.pop();

      self.main_view.end(ui);

      // Panel

      self.panel_view.begin(ui, input, Layout::Vertical);
      ui.fill_rounded(self.assets.colors.panel, ui.width() / 2.0);
      ui.pad(4.0);
      self.process_icon_panel(ui, input);
      self.panel_view.end(ui);

      // Language menu

      self.process_language_menu(ui, input);

      for message in &bus::retrieve_all::<Error>() {
         let error = message.consume().0;
         tracing::error!("error: {:?}", error);
         self.status = Status::Error(error.translate(&self.assets.language));
      }
      for message in &bus::retrieve_all::<Fatal>() {
         let fatal = message.consume().0;
         tracing::error!("fatal: {:?}", fatal);
         self.status = Status::Error(
            self
               .assets
               .tr
               .error_fatal
               .format()
               .with("error", fatal.translate(&self.assets.language))
               .done(),
         );
      }
   }

   fn next_state(self: Box<Self>, renderer: &mut Backend) -> Box<dyn AppState> {
      let mut connected = false;
      if let Some(peer) = &self.peer {
         for message in &bus::retrieve_all::<peer::Connected>() {
            tracing::info!("connection established");
            if message.peer == peer.token() {
               message.consume();
               connected = true;
            }
         }
      }

      if connected {
         let mut this = *self;
         let socket_system = Arc::clone(&this.socket_system);
         this.save_config();
         match paint::State::new(
            this.assets,
            this.socket_system,
            this.peer.unwrap(),
            this.image_file,
            renderer,
         ) {
            Ok(state) => Box::new(state),
            Err((error, assets)) => {
               bus::push(Fatal(error));
               Box::new(Self::new(assets, socket_system))
            }
         }
      } else {
         self
      }
   }

   fn exit(self: Box<Self>) {}
}
