use std::sync::Arc;

use nysa::global as bus;

use crate::app::{lobby, paint, AppState, StateArgs};
use crate::assets::Assets;
use crate::backend::Backend;
use crate::cli;
use crate::common::{Error, Fatal};
use crate::config::config;
use crate::net::{
   peer::{self, Peer},
   socket::SocketSystem,
};

pub struct State {
   assets: Box<Assets>,
   socket_system: Arc<SocketSystem>,
   peer: Option<Peer>,
}

impl State {
   pub fn new_state(
      cli: cli::Cli,
      assets: Box<Assets>,
      socket_system: Arc<SocketSystem>,
   ) -> Box<dyn AppState> {
      match cli.command {
         Some(cli::Commands::HostRoom { nickname, relay }) => {
            let nickname = nickname.unwrap_or(config().lobby.nickname.clone());
            let relay = relay.unwrap_or(config().lobby.relay.clone());
            let peer = Some(Peer::host(Arc::clone(&socket_system), &nickname, &relay));

            Box::new(Self {
               assets,
               socket_system,
               peer,
            })
         }
         Some(cli::Commands::JoinRoom {
            room_id,
            nickname,
            relay,
         }) => {
            let nickname = nickname.unwrap_or(config().lobby.nickname.clone());
            let relay = relay.unwrap_or(config().lobby.relay.clone());
            let peer = Some(Peer::join(
               Arc::clone(&socket_system),
               &nickname,
               &relay,
               room_id,
            ));

            Box::new(Self {
               assets,
               socket_system,
               peer,
            })
         }
         _ => Box::new(lobby::State::new(assets, Arc::clone(&socket_system))),
      }
   }
}

impl AppState for State {
   fn process(&mut self, _: StateArgs) {
      if let Some(peer) = &mut self.peer {
         catch!(peer.communicate());
      }

      for message in &bus::retrieve_all::<Error>() {
         let error = message.consume().0;
         tracing::error!("error: {:?}", error);
      }
      for message in &bus::retrieve_all::<Fatal>() {
         let fatal = message.consume().0;
         tracing::error!("fatal: {:?}", fatal);
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
         let this = *self;
         let socket_system = Arc::clone(&this.socket_system);
         match paint::State::new(
            this.assets,
            this.socket_system,
            this.peer.unwrap(),
            None,
            renderer,
         ) {
            Ok(state) => Box::new(state),
            Err((error, assets)) => {
               bus::push(Fatal(error));
               Box::new(Self {
                  assets,
                  socket_system,
                  peer: None,
               })
            }
         }
      } else {
         self
      }
   }

   fn exit(self: Box<Self>) {}
}
