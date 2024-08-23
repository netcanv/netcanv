//! The NetCanv Relay server.
//! Keeps track of open rooms and relays packets between peers.

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use log::LevelFilter;
use nanorand::Rng;
use netcanv_protocol::relay::{self, Packet, PeerId, RoomId, DEFAULT_PORT};
use simple_logger::SimpleLogger;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, tungstenite, WebSocketStream};

type Sink = SplitSink<WebSocketStream<TcpStream>, Message>;
type Stream = SplitStream<WebSocketStream<TcpStream>>;

#[derive(StructOpt)]
#[structopt(name = "netcanv-relay")]
struct Options {
   /// The port to host the relay under.
   #[structopt(short)]
   port: Option<u16>,
}

struct Rooms {
   occupied_room_ids: HashSet<RoomId>,
   client_rooms: HashMap<PeerId, RoomId>,
   room_clients: HashMap<RoomId, Vec<PeerId>>,
   room_hosts: HashMap<RoomId, PeerId>,
}

impl Rooms {
   /// The room ID character set. Room IDs are composed of characters picked at random from
   /// this string.
   ///
   /// This is _almost_ base32, with `I`, `0`, and `O` omitted to avoid confusion.
   /// Some fonts render `0` and `O` in a very similar way, and people often confuse the capital
   /// `I` for the lowercase `l`, even if it's not a part of a code.
   ///
   /// **Warning:** all characters in this string must be ASCII, as [`Self::generate_room_id`] does
   /// not handle Unicode characters for performance reasons.
   const ID_CHARSET: &'static [u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZ";

   fn new() -> Self {
      Self {
         occupied_room_ids: HashSet::new(),
         client_rooms: HashMap::new(),
         room_clients: HashMap::new(),
         room_hosts: HashMap::new(),
      }
   }

   /// Generates a pseudo-random room ID.
   fn generate_room_id(&self) -> RoomId {
      let mut rng = nanorand::tls_rng();
      RoomId([(); 6].map(|_| {
         let index = rng.generate_range(0..Self::ID_CHARSET.len());
         Self::ID_CHARSET[index]
      }))
   }

   /// Allocates a new, free room ID.
   ///
   /// Returns `None` if all attempts to find a free ID have failed.
   fn find_room_id(&mut self) -> Option<RoomId> {
      for _attempt in 0..50 {
         let id = self.generate_room_id();
         if self.occupied_room_ids.insert(id) {
            self.room_clients.insert(id, Vec::new());
            return Some(id);
         }
      }
      None
   }

   /// Makes the peer with the given ID the host of this room.
   fn make_host(&mut self, room_id: RoomId, peer_id: PeerId) {
      self.room_hosts.insert(room_id, peer_id);
   }

   /// Makes the peer join the room with the given ID.
   fn join_room(&mut self, peer_id: PeerId, room_id: RoomId) {
      if let Some(room_clients) = self.room_clients.get_mut(&room_id) {
         self.client_rooms.insert(peer_id, room_id);
         room_clients.push(peer_id);
      }
   }

   /// Removes a room.
   fn remove_room(&mut self, room_id: RoomId) {
      self.occupied_room_ids.remove(&room_id);
      self.room_clients.remove(&room_id);
      self.room_hosts.remove(&room_id);
   }

   /// Makes the peer quit the room with the given ID. Returns the peer's room ID.
   fn quit_room(&mut self, peer_id: PeerId) {
      if let Some(room_id) = self.client_rooms.remove(&peer_id) {
         let n_connected = if let Some(room_clients) = self.room_clients.get_mut(&room_id) {
            if let Some(index) = room_clients.iter().position(|&id| id == peer_id) {
               // We use the order-preserving `remove`, such that peers are queued up for the host
               // role in the order they joined into the room.
               room_clients.remove(index);
            }
            room_clients.len()
         } else {
            0
         };
         if n_connected == 0 {
            self.remove_room(room_id);
         }
      }
   }

   /// Returns the ID of the given room's host, or `None` if the room doesn't exist.
   fn host_id(&self, room_id: RoomId) -> Option<PeerId> {
      self.room_hosts.get(&room_id).cloned()
   }

   /// Returns the ID of the given peer's room, or `None` if they haven't joined a room yet.
   fn room_id(&self, peer_id: PeerId) -> Option<RoomId> {
      self.client_rooms.get(&peer_id).cloned()
   }

   /// Returns an iterator over all the peers in a given room.
   fn peers_in_room(&self, room_id: RoomId) -> Option<impl Iterator<Item = PeerId> + '_> {
      Some(self.room_clients.get(&room_id)?.iter().cloned())
   }
}

struct Peers {
   occupied_peer_ids: HashSet<PeerId>,
   peer_ids: HashMap<SocketAddr, PeerId>,
   peer_sinks: HashMap<PeerId, Arc<Mutex<Sink>>>,
}

impl Peers {
   fn new() -> Self {
      Self {
         occupied_peer_ids: HashSet::new(),
         peer_ids: HashMap::new(),
         peer_sinks: HashMap::new(),
      }
   }

   /// Allocates a new peer ID for the given socket address.
   fn allocate_peer_id(&mut self, sink: Arc<Mutex<Sink>>, address: SocketAddr) -> Option<PeerId> {
      let mut rng = nanorand::tls_rng();
      for _attempt in 0..50 {
         let id = PeerId(rng.generate_range(PeerId::FIRST_PEER..=PeerId::LAST_PEER));
         if self.occupied_peer_ids.insert(id) {
            self.peer_ids.insert(address, id);
            self.peer_sinks.insert(id, sink);
            return Some(id);
         }
      }
      None
   }

   /// Deallocates the peer with the given ID. New peers will be able to join with the same ID.
   fn free_peer_id(&mut self, address: SocketAddr) {
      if let Some(id) = self.peer_ids.remove(&address) {
         self.occupied_peer_ids.remove(&id);
         self.peer_sinks.remove(&id);
      }
   }

   /// Returns the ID of the peer with the given socket address.
   fn peer_id(&self, address: SocketAddr) -> Option<PeerId> {
      self.peer_ids.get(&address).cloned()
   }
}

struct State {
   rooms: Rooms,
   peers: Peers,
}

impl State {
   fn new() -> Self {
      Self {
         rooms: Rooms::new(),
         peers: Peers::new(),
      }
   }
}

async fn send_packet(sink: &Mutex<Sink>, packet: Packet) -> anyhow::Result<()> {
   let encoded = bincode::serialize(&packet)?;
   let mut sink = sink.lock().await;
   u32::try_from(encoded.len()).context("packet is too big")?;

   sink.send(Message::Binary(encoded)).await?;
   Ok(())
}

/// Broadcasts a packet to all peers in the room.
///
/// If `sender` is not `PeerId::BROADCAST`, the packet is not sent to them.
async fn broadcast_packet(
   state: &mut State,
   room_id: RoomId,
   sender_id: PeerId,
   packet: Packet,
) -> anyhow::Result<()> {
   let packet = bincode::serialize(&packet)?;
   u32::try_from(packet.len()).context("packet is too big")?;

   let peers_in_room = state.rooms.peers_in_room(room_id);
   let mut result = Ok(());
   if let Some(iter) = peers_in_room {
      for peer_id in iter {
         if peer_id != sender_id {
            if let Some(stream) = state.peers.peer_sinks.get(&peer_id) {
               match stream.lock().await.send(Message::Binary(packet.to_owned())).await {
                  Ok(()) => (),
                  Err(error) => result = Err(error),
               }
            }
         }
      }
   }
   Ok(result?)
}

async fn host(
   write: &Arc<Mutex<Sink>>,
   address: SocketAddr,
   state: &mut State,
) -> anyhow::Result<()> {
   let peer_id = if let Some(id) = state.peers.allocate_peer_id(Arc::clone(write), address) {
      id
   } else {
      send_packet(write, Packet::Error(relay::Error::NoFreePeerIDs)).await?;
      anyhow::bail!("no more free peer IDs");
   };

   let room_id = if let Some(id) = state.rooms.find_room_id() {
      id
   } else {
      send_packet(write, Packet::Error(relay::Error::NoFreeRooms)).await?;
      anyhow::bail!("no more free room IDs");
   };

   state.rooms.make_host(room_id, peer_id);
   state.rooms.join_room(peer_id, room_id);
   send_packet(write, Packet::RoomCreated(room_id, peer_id)).await?;

   Ok(())
}

async fn join(
   write: &Arc<Mutex<Sink>>,
   address: SocketAddr,
   state: &mut State,
   room_id: RoomId,
) -> anyhow::Result<()> {
   let peer_id = if let Some(id) = state.peers.allocate_peer_id(Arc::clone(write), address) {
      id
   } else {
      send_packet(write, Packet::Error(relay::Error::NoFreePeerIDs)).await?;
      anyhow::bail!("no more free peer IDs");
   };

   let host_id = if let Some(id) = state.rooms.host_id(room_id) {
      id
   } else {
      send_packet(write, Packet::Error(relay::Error::RoomDoesNotExist)).await?;
      anyhow::bail!("no room with the given ID");
   };

   state.rooms.join_room(peer_id, room_id);
   send_packet(write, Packet::Joined { peer_id, host_id }).await?;

   Ok(())
}

/// Relays a packet to the peer with the given ID.
async fn relay(
   write: &Mutex<Sink>,
   address: SocketAddr,
   state: &mut State,
   target_id: PeerId,
   data: Vec<u8>,
) -> anyhow::Result<()> {
   let sender_id =
      state.peers.peer_id(address).ok_or_else(|| anyhow::anyhow!("peer does not have an ID"))?;
   let room_id =
      state.rooms.room_id(sender_id).ok_or_else(|| anyhow::anyhow!("peer is not in a room"))?;

   let packet = Packet::Relayed(sender_id, data);
   if target_id.is_broadcast() {
      broadcast_packet(state, room_id, sender_id, packet).await?;
   } else if let Some(sink) = state.peers.peer_sinks.get(&target_id) {
      send_packet(sink, packet).await?;
   } else {
      send_packet(
         write,
         Packet::Error(relay::Error::NoSuchPeer { address: target_id }),
      )
      .await?;
   }

   Ok(())
}

async fn handle_packet(
   write: &Arc<Mutex<Sink>>,
   address: SocketAddr,
   state: &Mutex<State>,
   packet: Packet,
) -> anyhow::Result<()> {
   match packet {
      Packet::Host => host(write, address, &mut *state.lock().await).await?,
      Packet::Join(room_id) => join(write, address, &mut *state.lock().await, room_id).await?,
      Packet::Relay(target_id, data) => {
         relay(write, address, &mut *state.lock().await, target_id, data).await?
      }

      // These ones shouldn't happen, ignore.
      Packet::RoomCreated(_room_id, _peer_id) => (),
      Packet::Joined { .. } => (),
      Packet::HostTransfer(_host_id) => (),
      Packet::Relayed(_peer_id, _data) => (),
      Packet::Disconnected(_peer_id) => (),
      Packet::Error(_message) => (),
   }
   Ok(())
}

async fn read_packets(
   mut read: Stream,
   write: Arc<Mutex<Sink>>,
   address: SocketAddr,
   state: &Mutex<State>,
) -> anyhow::Result<()> {
   while let Some(message) = read.next().await {
      match message {
         Ok(Message::Binary(buffer)) => {
            if buffer.len() > relay::MAX_PACKET_SIZE as usize {
               anyhow::bail!("packet is too big");
            }
            let packet = bincode::deserialize(&buffer)?;
            handle_packet(&write, address, state, packet).await?;
         }
         Ok(Message::Close(frame)) => {
            if let Some(frame) = frame {
               log::info!("client disconnected, reason: {}", frame.reason);
               return Ok(());
            }
         }
         Ok(Message::Pong(_)) => {}
         Ok(_) => log::info!("got ignored message"),
         Err(e) => {
            use tungstenite::Error::*;
            match e {
               ConnectionClosed => break,
               AlreadyClosed => {
                  // According to the documentation this error is the fault of the programmer.
                  // However, this error would crash the entire relay and *all* rooms,
                  // so it's better to treat it as a simple error and end the connection.
                  log::error!("cannot work with already closed connection");
                  break;
               }
               _ => anyhow::bail!(e),
            }
         }
      }
   }

   Ok(())
}

/// Performs the host transferrence procedure.
///
/// This transfers the host status to the next person that joined the room.
async fn transfer_host(state: &mut State, room_id: RoomId) -> anyhow::Result<()> {
   // If we get here, the room can't have been deleted, and because of that, there's at least
   // one person still in the room.
   let new_host_id = state.rooms.peers_in_room(room_id).unwrap().next().unwrap();
   state.rooms.make_host(room_id, new_host_id);
   broadcast_packet(
      state,
      room_id,
      PeerId::BROADCAST,
      Packet::HostTransfer(new_host_id),
   )
   .await?;
   Ok(())
}

/// Pings the sink periodically.
async fn ping_loop(write: Arc<Mutex<Sink>>) -> anyhow::Result<()> {
   // This loop is exited whenever the stream is closed.
   const PING_MESSAGE: &str = concat!("PING NetCanv Relay ", env!("CARGO_PKG_VERSION"));
   const PING_PERIOD: Duration = Duration::from_secs(5);
   loop {
      tokio::time::sleep(PING_PERIOD).await;
      write.lock().await.send(Message::Ping(PING_MESSAGE.as_bytes().to_owned())).await?;
   }
}

async fn handle_connection(
   stream: TcpStream,
   address: SocketAddr,
   state: Arc<Mutex<State>>,
) -> anyhow::Result<()> {
   log::info!("{} has connected", address);
   stream.set_nodelay(true)?;

   let (mut write, read) = {
      let stream = accept_async(stream).await?;
      stream.split()
   };

   let version = relay::PROTOCOL_VERSION.to_le_bytes();
   write.send(tungstenite::Message::binary(version)).await?;
   let write = Arc::new(Mutex::new(write));

   let pinger = {
      let write = Arc::clone(&write);
      tokio::spawn(async move {
         if let Err(error) = ping_loop(write).await {
            log::error!("[{}] ping loop: {}", address, error);
         }
      })
   };

   match read_packets(read, write, address, &state).await {
      Ok(()) => (),
      Err(error) => log::error!("[{}] connection error: {}", address, error),
   }

   // Abort the pinger if it hasn't already exited.
   pinger.abort();

   log::info!("tearing down {}'s connection", address);
   {
      let mut state = state.lock().await;
      let peer_id =
         state.peers.peer_id(address).ok_or_else(|| anyhow::anyhow!("peer had no ID"))?;
      let room_id = state.rooms.room_id(peer_id);
      state.rooms.quit_room(peer_id);
      if let Some(room_id) = room_id {
         broadcast_packet(
            &mut state,
            room_id,
            PeerId::BROADCAST,
            Packet::Disconnected(peer_id),
         )
         .await?;
         if state.rooms.host_id(room_id) == Some(peer_id) {
            transfer_host(&mut state, room_id).await?;
         }
      }
      state.peers.free_peer_id(address);
   }

   Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
   SimpleLogger::new().with_level(LevelFilter::Debug).env().init()?;
   let options = Options::from_args();

   let listener = TcpListener::bind((
      Ipv4Addr::from([0, 0, 0, 0]),
      options.port.unwrap_or(DEFAULT_PORT),
   ))
   .await?;
   let state = Arc::new(Mutex::new(State::new()));

   log::info!(
      "NetCanv Relay server {} (protocol version {})",
      env!("CARGO_PKG_VERSION"),
      relay::PROTOCOL_VERSION
   );
   log::info!("listening on {}", listener.local_addr()?);

   loop {
      let (socket, address) = listener.accept().await?;
      let state = Arc::clone(&state);
      tokio::spawn(async move { handle_connection(socket, address, state).await });
   }
}
