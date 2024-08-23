use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use netcanv_protocol::relay::{PeerId, RoomId};
use netcanv_protocol::{client as cl, relay};
use nysa::global as bus;
use tokio::sync::oneshot;

use super::socket::{Socket, SocketSystem};
use crate::common::{deserialize_bincode, serialize_bincode, Fatal};
use crate::token::Token;
use crate::Error;

/// A unique token identifying a peer connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerToken(usize);

/// A message that a peer connection has been successfully established.
pub struct Connected {
   pub peer: PeerToken,
}

/// A bus message from a peer connection.
pub struct Message {
   pub token: PeerToken,
   pub kind: MessageKind,
}

/// The data associated with a peer message.
pub enum MessageKind {
   /// Another peer has joined the room.
   Joined(String, PeerId),
   /// Another peer has left the room.
   Left {
      peer_id: PeerId,
      nickname: String,
      last_tool: Option<String>,
   },
   /// The host role has been transferred to another peer in the room.
   NewHost(String),
   /// The host role has been transferred to the peer (you).
   NowHosting,
   /// The host sent us the chunk positions for the room.
   ChunkPositions(Vec<(i32, i32)>),
   /// Somebody requested chunk positions from the host.
   GetChunks(PeerId, Vec<(i32, i32)>),
   /// Somebody sent us chunk image data.
   Chunks(Vec<((i32, i32), Vec<u8>)>),
   /// A tool packet was received from an address.
   Tool(PeerId, String, Vec<u8>),
   /// The client selected a tool.
   SelectTool {
      peer_id: PeerId,
      previous_tool: Option<String>,
      tool: String,
   },
}

/// Another person in the same room.
pub struct Mate {
   pub nickname: String,
   pub tool: Option<String>,
}

enum State {
   WaitingForRelay(oneshot::Receiver<netcanv::Result<Socket>>),
   ConnectedToRelay,
   InRoom,
}

/// A connection to the relay.
pub struct Peer {
   token: PeerToken,
   state: State,
   relay_socket: Option<Socket>,

   is_host: bool,

   nickname: String,
   room_id: Option<RoomId>,
   peer_id: Option<PeerId>,
   host: Option<PeerId>,
   mates: HashMap<PeerId, Mate>,
}

static PEER_TOKEN: Token = Token::new(0);

impl Peer {
   /// Host a new room on the given relay server.
   pub fn host(socket_system: Arc<SocketSystem>, nickname: &str, relay_address: &str) -> Self {
      let socket_receiver = socket_system.connect(relay_address.to_owned());
      Self {
         token: PeerToken(PEER_TOKEN.next()),
         state: State::WaitingForRelay(socket_receiver),
         relay_socket: None,
         is_host: true,
         nickname: nickname.into(),
         room_id: None,
         peer_id: None,
         mates: HashMap::new(),
         host: None,
      }
   }

   /// Join an existing room on the given relay server.
   pub fn join(
      socket_system: Arc<SocketSystem>,
      nickname: &str,
      relay_address: &str,
      room_id: RoomId,
   ) -> Self {
      let socket_receiver = socket_system.connect(relay_address.to_owned());
      Self {
         token: PeerToken(PEER_TOKEN.next()),
         state: State::WaitingForRelay(socket_receiver),
         relay_socket: None,
         is_host: false,
         nickname: nickname.into(),
         room_id: Some(room_id),
         peer_id: None,
         mates: HashMap::new(),
         host: None,
      }
   }

   /// Sends a relay packet to the currently connected relay, or fails if there's no
   /// relay connection.
   fn send_to_relay(&self, packet: relay::Packet) -> netcanv::Result<()> {
      match &self.state {
         State::ConnectedToRelay | State::InRoom => {
            self.relay_socket.as_ref().unwrap().send(packet);
         }
         _ => return Err(Error::NotConnectedToRelay),
      }
      Ok(())
   }

   /// Sends a client packet to the peer with the given address.
   fn send_to_client(&self, to: PeerId, packet: cl::Packet) -> netcanv::Result<()> {
      match &self.state {
         State::InRoom => {
            self.send_to_relay(relay::Packet::Relay(to, serialize_bincode(&packet)?))?;
         }
         _ => return Err(Error::NotConnectedToHost),
      }
      Ok(())
   }

   /// Sends a message onto the global bus.
   fn send_message(&self, message: MessageKind) {
      bus::push(Message {
         token: self.token,
         kind: message,
      })
   }

   /// Checks the message bus for any established connections.
   fn poll_for_new_connections(&mut self) -> netcanv::Result<()> {
      if let State::WaitingForRelay(socket) = &mut self.state {
         if let Ok(socket) = socket.try_recv() {
            let socket = catch!(socket, as Fatal, return Ok(()));
            self.connected_to_relay(socket)?;
         }
      }
      Ok(())
   }

   /// Handles the state transition from connecting to the relay to being connected to the
   /// relay.
   ///
   /// In the process, sends the appropriate packet to the relay - whether to host or join a
   /// room.
   fn connected_to_relay(&mut self, socket: Socket) -> netcanv::Result<()> {
      self.state = State::ConnectedToRelay;
      tracing::info!("connected to relay");
      self.relay_socket = Some(socket);
      self.send_to_relay(if self.is_host {
         relay::Packet::Host
      } else {
         relay::Packet::Join(self.room_id.unwrap())
      })?;
      Ok(())
   }

   /// Polls for any incoming packets.
   fn poll_for_incoming_packets(&mut self) -> netcanv::Result<()> {
      match &self.state {
         State::WaitingForRelay(_) => (),
         State::ConnectedToRelay | State::InRoom => {
            while let Some(packet) = self.relay_socket.as_mut().unwrap().recv() {
               self.relay_packet(packet)?;
            }
         }
      }
      Ok(())
   }

   /// Handles a relay packet.
   fn relay_packet(&mut self, packet: relay::Packet) -> netcanv::Result<()> {
      match packet {
         relay::Packet::RoomCreated(room_id, peer_id) => {
            tracing::info!("got free room ID: {:?}", room_id);
            self.room_id = Some(room_id);
            self.peer_id = Some(peer_id);
            self.state = State::InRoom;
            bus::push(Connected { peer: self.token });
         }
         relay::Packet::Joined { peer_id, host_id } => {
            tracing::info!("got host ID: {:?}", host_id);
            self.peer_id = Some(peer_id);
            self.host = Some(host_id);
            self.state = State::InRoom;
            bus::push(Connected { peer: self.token });
            self.say_hello()?;
         }
         relay::Packet::HostTransfer(host_id) => {
            if self.peer_id == Some(host_id) {
               self.send_message(MessageKind::NowHosting);
               self.host = None;
               self.is_host = true;
            } else {
               if let Some(mate) = self.mates.get(&host_id) {
                  self.send_message(MessageKind::NewHost(mate.nickname.clone()))
               }
               self.host = Some(host_id);
            }
         }
         relay::Packet::Relayed(author, payload) => {
            let client_packet: cl::Packet = deserialize_bincode(&payload)?;
            self.client_packet(author, client_packet)?;
         }
         relay::Packet::Disconnected(address) => {
            self.remove_mate(address);
         }
         relay::Packet::Error(error) => match error {
            relay::Error::NoSuchPeer { address } => {
               // Remove the peer when relay tells us that they are no longer
               // in the room.
               //
               // This is fine, because the relay already knows this, so what
               // we are doing here is synchronising our state with relay's state.
               if self.mates.contains_key(&address) {
                  self.remove_mate(address);
                  tracing::warn!(
                     "{:?} is no longer in the room, so they got removed",
                     address
                  );
               }
            }
            _ => return Err(Error::Relay(error)),
         },
         _ => return Err(Error::UnexpectedRelayPacket),
      }
      Ok(())
   }

   /// Says hello to other peers in the room.
   fn say_hello(&self) -> netcanv::Result<()> {
      self.send_to_client(PeerId::BROADCAST, cl::Packet::Hello(self.nickname.clone()))
   }

   /// Decodes a client packet.
   fn client_packet(&mut self, author: PeerId, packet: cl::Packet) -> netcanv::Result<()> {
      match packet {
         // -----
         // 0.1.0
         // -----
         cl::Packet::Hello(nickname) => {
            tracing::info!("{} ({:?}) joined", nickname, author);
            self.send_to_client(author, cl::Packet::HiThere(self.nickname.clone()))?;
            self.send_to_client(author, cl::Packet::Version(cl::PROTOCOL_VERSION))?;
            self.add_mate(author, nickname.clone());
            self.send_message(MessageKind::Joined(nickname, author));
         }
         cl::Packet::HiThere(nickname) => {
            tracing::info!("{} ({:?}) is in the room", nickname, author);
            self.add_mate(author, nickname);
         }
         cl::Packet::Reserved1 => (),
         // -----
         // 0.2.0
         // -----
         cl::Packet::Version(version) if !cl::compatible_with(version) => {
            bus::push(Fatal(match cl::PROTOCOL_VERSION.cmp(&version) {
               Ordering::Less => Error::ClientIsTooOld,
               Ordering::Greater => Error::ClientIsTooNew,
               Ordering::Equal => unreachable!(),
            }));
         }
         cl::Packet::Version(_) => (),
         cl::Packet::ChunkPositions(positions) => {
            self.send_message(MessageKind::ChunkPositions(positions))
         }
         cl::Packet::GetChunks(positions) => {
            self.send_message(MessageKind::GetChunks(author, positions))
         }
         cl::Packet::Chunks(chunks) => self.send_message(MessageKind::Chunks(chunks)),
         // -----
         // 0.3.0
         // -----
         cl::Packet::Tool(name, payload) => {
            self.send_message(MessageKind::Tool(author, name, payload))
         }
         cl::Packet::SelectTool(tool) => {
            let mut old_tool = None;
            if let Some(mate) = self.mates.get_mut(&author) {
               old_tool = std::mem::replace(&mut mate.tool, Some(tool.clone()));
            }
            self.send_message(MessageKind::SelectTool {
               peer_id: author,
               previous_tool: old_tool,
               tool,
            });
         }
      }

      Ok(())
   }

   /// Ticks the peer's network connection.
   pub fn communicate(&mut self) -> netcanv::Result<()> {
      self.poll_for_new_connections()?;
      self.poll_for_incoming_packets()?;
      Ok(())
   }

   /// Adds another peer into the list of registered peers.
   fn add_mate(&mut self, peer_id: PeerId, nickname: String) {
      self.mates.insert(
         peer_id,
         Mate {
            nickname,
            tool: None,
         },
      );
   }

   /// Removes a peer from the list of registered peers
   /// and sends to everyone that they left.
   pub fn remove_mate(&mut self, peer_id: PeerId) {
      if let Some(mate) = self.mates.remove(&peer_id) {
         self.send_message(MessageKind::Left {
            peer_id,
            nickname: mate.nickname,
            last_tool: mate.tool,
         });
      }
   }

   /// Sends a chunk positions packet.
   pub fn send_chunk_positions(
      &self,
      to: PeerId,
      positions: Vec<(i32, i32)>,
   ) -> netcanv::Result<()> {
      self.send_to_client(to, cl::Packet::ChunkPositions(positions))
   }

   /// Requests chunk data from the host.
   pub fn download_chunks(&self, positions: Vec<(i32, i32)>) -> netcanv::Result<()> {
      assert!(self.host.is_some(), "only non-hosts can download chunks");
      tracing::info!("downloading {} chunks from the host", positions.len());
      // The host should be available at this point, as the connection has been established.
      self.send_to_client(self.host.unwrap(), cl::Packet::GetChunks(positions))
   }

   /// Sends chunks to the given peer.
   pub fn send_chunks(
      &self,
      to: PeerId,
      chunks: Vec<((i32, i32), Vec<u8>)>,
   ) -> netcanv::Result<()> {
      self.send_to_client(to, cl::Packet::Chunks(chunks))
   }

   /// Sends a tool-specific packet.
   pub fn send_tool(&self, peer_id: PeerId, name: String, payload: Vec<u8>) -> netcanv::Result<()> {
      self.send_to_client(peer_id, cl::Packet::Tool(name, payload))
   }

   /// Sends a tool selection packet.
   pub fn send_select_tool(&self, name: String) -> netcanv::Result<()> {
      self.send_to_client(PeerId::BROADCAST, cl::Packet::SelectTool(name))
   }

   /// Returns the peer's unique token.
   pub fn token(&self) -> PeerToken {
      self.token
   }

   /// Returns whether this peer is the host.
   pub fn is_host(&self) -> bool {
      self.is_host
   }

   /// Returns the name of the host, or `None` if this peer is the host (or if the host's name isn't
   /// yet known).
   pub fn host_name(&self) -> Option<&str> {
      if self.is_host() {
         None
      } else if let Some(mate) = self.mates.get(&self.host?) {
         Some(&mate.nickname)
      } else {
         None
      }
   }

   /// Returns the ID of the room, or `None` if a connection hasn't been established.
   pub fn room_id(&self) -> Option<RoomId> {
      self.room_id
   }

   /// Returns the list of peers connected to the same room.
   pub fn mates(&self) -> &HashMap<PeerId, Mate> {
      &self.mates
   }
}
