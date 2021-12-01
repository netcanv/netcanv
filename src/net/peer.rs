use std::collections::HashMap;
use std::mem;
use std::sync::Arc;

use netcanv_protocol::matchmaker::{PeerId, RoomId};
use netcanv_protocol::{client as cl, matchmaker as mm};
use nysa::global as bus;

use super::socket::{self, ConnectionToken, Socket, SocketSystem};
use crate::common::Fatal;
use crate::token::Token;

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

/// The state of a Peer connection.
#[derive(Debug)]
enum State {
   // No connection has been established yet. We're waiting on the socket subsystem to give us a socket.
   WaitingForMatchmaker { token: ConnectionToken },
   // We're connected to the matchmaker, but haven't obtained the other person's connection
   // details yet.
   ConnectedToMatchmaker,
   // We're connected to a host.
   InRoom,
}

/// Another person in the same room.
pub struct Mate {
   pub nickname: String,
   pub tool: Option<String>,
}

/// A connection to the matchmaker.
pub struct Peer {
   token: PeerToken,
   state: State,
   matchmaker_socket: Option<Socket<mm::Packet>>,

   is_host: bool,

   nickname: String,
   room_id: Option<RoomId>,
   peer_id: Option<PeerId>,
   host: Option<PeerId>,
   mates: HashMap<PeerId, Mate>,
}

static PEER_TOKEN: Token = Token::new();

impl Peer {
   /// Host a new room on the given matchmaker.
   pub fn host(
      socksys: &Arc<SocketSystem<mm::Packet>>,
      nickname: &str,
      matchmaker_address: &str,
   ) -> anyhow::Result<Self> {
      let connection_token = socksys.connect(matchmaker_address.to_owned(), mm::DEFAULT_PORT)?;
      Ok(Self {
         token: PeerToken(PEER_TOKEN.next()),
         state: State::WaitingForMatchmaker {
            token: connection_token,
         },
         matchmaker_socket: None,
         is_host: true,
         nickname: nickname.into(),
         room_id: None,
         peer_id: None,
         mates: HashMap::new(),
         host: None,
      })
   }

   /// Join an existing room on the given matchmaker.
   pub fn join(
      socksys: &Arc<SocketSystem<mm::Packet>>,
      nickname: &str,
      matchmaker_address: &str,
      room_id: RoomId,
   ) -> anyhow::Result<Self> {
      let connection_token = socksys.connect(matchmaker_address.to_owned(), mm::DEFAULT_PORT)?;
      Ok(Self {
         token: PeerToken(PEER_TOKEN.next()),
         state: State::WaitingForMatchmaker {
            token: connection_token,
         },
         matchmaker_socket: None,
         is_host: false,
         nickname: nickname.into(),
         room_id: Some(room_id),
         peer_id: None,
         mates: HashMap::new(),
         host: None,
      })
   }

   /// Returns the connection token of the matchmaker socket.
   fn matchmaker_token(&self) -> Option<ConnectionToken> {
      self.matchmaker_socket.as_ref().map(|socket| socket.token())
   }

   /// Sends a matchmaker packet to the currently connected matchmaker, or fails if there's no
   /// matchmaker connection.
   fn send_to_matchmaker(&self, packet: mm::Packet) -> anyhow::Result<()> {
      match &self.state {
         State::ConnectedToMatchmaker | State::InRoom => {
            self.matchmaker_socket.as_ref().unwrap().send(packet)
         }
         _ => anyhow::bail!("cannot send packet: not connected to the matchmaker"),
      }
      Ok(())
   }

   /// Sends a client packet to the peer with the given address.
   fn send_to_client(&self, to: PeerId, packet: cl::Packet) -> anyhow::Result<()> {
      match &self.state {
         State::InRoom => {
            self.send_to_matchmaker(mm::Packet::Relay(to, bincode::serialize(&packet)?))?;
         }
         _ => anyhow::bail!("cannot send packet: not connected to the host"),
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
   fn poll_for_new_connections(&mut self) -> anyhow::Result<()> {
      for message in &bus::retrieve_all::<socket::Connected<mm::Packet>>() {
         match self.state {
            // If a new connection was established and we're trying to connect to a matchmaker, check if the
            // connection is ours.
            State::WaitingForMatchmaker { token } if message.token == token => {
               let socket = message.consume().socket;
               self.connected_to_matchmaker(socket)?;
            }
            _ => (),
         }
      }
      Ok(())
   }

   /// Handles the state transition from connecting to the matchmaker to being connected to the
   /// matchmaker.
   ///
   /// In the process, sends the appropriate packet to the matchmaker - whether to host or join a
   /// room.
   fn connected_to_matchmaker(&mut self, socket: Socket<mm::Packet>) -> anyhow::Result<()> {
      let state = mem::replace(&mut self.state, State::ConnectedToMatchmaker);
      println!("connected to matchmaker");
      self.matchmaker_socket = Some(socket);
      match state {
         State::WaitingForMatchmaker { .. } => self.send_to_matchmaker(if self.is_host {
            mm::Packet::Host
         } else {
            mm::Packet::Join(self.room_id.unwrap())
         }),
         _ => unreachable!(),
      }
   }

   /// Polls for any incoming packets.
   fn poll_for_incoming_packets(&mut self) -> anyhow::Result<()> {
      for message in &bus::retrieve_all::<socket::IncomingPacket<mm::Packet>>() {
         match &self.state {
            // Ignore incoming packets if no socket connection is open yet.
            // These packets may be coming in, but they're not for us.
            State::WaitingForMatchmaker { .. } => (),
            State::ConnectedToMatchmaker | State::InRoom
               if Some(message.token) == self.matchmaker_token() =>
            {
               let packet = message.consume().data;
               self.matchmaker_packet(packet)?;
            }
            _ => (),
         }
      }
      Ok(())
   }

   /// Handles a matchmaker packet.
   fn matchmaker_packet(&mut self, packet: mm::Packet) -> anyhow::Result<()> {
      match packet {
         mm::Packet::RoomCreated(room_id, peer_id) => {
            eprintln!("got free room ID: {:?}", room_id);
            self.room_id = Some(room_id);
            self.peer_id = Some(peer_id);
            self.state = State::InRoom;
            bus::push(Connected { peer: self.token });
         }
         mm::Packet::Joined { peer_id, host_id } => {
            eprintln!("got host ID: {:?}", host_id);
            self.peer_id = Some(peer_id);
            self.host = Some(host_id);
            self.state = State::InRoom;
            bus::push(Connected { peer: self.token });
            self.say_hello()?;
         }
         mm::Packet::HostTransfer(host_id) => {
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
         mm::Packet::Relayed(author, payload) => {
            let client_packet: cl::Packet = bincode::deserialize(&payload)?;
            self.client_packet(author, client_packet)?;
         }
         mm::Packet::Disconnected(address) => {
            if let Some(mate) = self.mates.remove(&address) {
               self.send_message(MessageKind::Left {
                  peer_id: address,
                  nickname: mate.nickname,
                  last_tool: mate.tool,
               });
            }
         }
         mm::Packet::Error(error) => anyhow::bail!("{}", matchmaker_error_to_string(error)),
         other => anyhow::bail!("unexpected matchmaker packet: {:?}", other),
      }
      Ok(())
   }

   /// Says hello to other peers in the room.
   fn say_hello(&self) -> anyhow::Result<()> {
      self.send_to_client(PeerId::BROADCAST, cl::Packet::Hello(self.nickname.clone()))
   }

   /// Decodes a client packet.
   fn client_packet(&mut self, author: PeerId, packet: cl::Packet) -> anyhow::Result<()> {
      match packet {
         // -----
         // 0.1.0
         // -----
         cl::Packet::Hello(nickname) => {
            eprintln!("{} ({:?}) joined", nickname, author);
            self.send_to_client(author, cl::Packet::HiThere(self.nickname.clone()))?;
            self.send_to_client(author, cl::Packet::Version(cl::PROTOCOL_VERSION))?;
            self.add_mate(author, nickname.clone());
            self.send_message(MessageKind::Joined(nickname, author));
         }
         cl::Packet::HiThere(nickname) => {
            eprintln!("{} ({:?}) is in the room", nickname, author);
            self.add_mate(author, nickname);
         }
         cl::Packet::Reserved1 => (),
         // -----
         // 0.2.0
         // -----
         cl::Packet::Version(version) if !cl::compatible_with(version) => {
            bus::push(Fatal(anyhow::anyhow!("Client is too old.")));
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
   pub fn communicate(&mut self) -> anyhow::Result<()> {
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

   /// Sends a chunk positions packet.
   pub fn send_chunk_positions(
      &self,
      to: PeerId,
      positions: Vec<(i32, i32)>,
   ) -> anyhow::Result<()> {
      self.send_to_client(to, cl::Packet::ChunkPositions(positions))
   }

   /// Requests chunk data from the host.
   pub fn download_chunks(&self, positions: Vec<(i32, i32)>) -> anyhow::Result<()> {
      assert!(self.host.is_some(), "only non-hosts can download chunks");
      eprintln!("downloading {} chunks from the host", positions.len());
      // The host should be available at this point, as the connection has been established.
      self.send_to_client(self.host.unwrap(), cl::Packet::GetChunks(positions))
   }

   /// Sends chunks to the given peer.
   pub fn send_chunks(&self, to: PeerId, chunks: Vec<((i32, i32), Vec<u8>)>) -> anyhow::Result<()> {
      self.send_to_client(to, cl::Packet::Chunks(chunks))
   }

   /// Sends a tool-specific packet.
   pub fn send_tool(&self, peer_id: PeerId, name: String, payload: Vec<u8>) -> anyhow::Result<()> {
      self.send_to_client(peer_id, cl::Packet::Tool(name, payload))
   }

   /// Sends a tool selection packet.
   pub fn send_select_tool(&self, name: String) -> anyhow::Result<()> {
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

   /// Returns the ID of the room, or `None` if a connection hasn't been established.
   pub fn room_id(&self) -> Option<RoomId> {
      self.room_id
   }

   /// Returns the list of peers connected to the same room.
   pub fn mates(&self) -> &HashMap<PeerId, Mate> {
      &self.mates
   }
}

fn matchmaker_error_to_string(error: mm::Error) -> &'static str {
   match error {
      mm::Error::NoFreeRooms => "Could not find any more free rooms. Try again",
      // Hopefully this one never happens.
      mm::Error::NoFreePeerIDs => "The matchmaker is full. Try a different server",
      mm::Error::RoomDoesNotExist => {
         "No room with the given ID. Check if you spelled the ID correctly"
      }
      // This one also shouldn't happen.
      mm::Error::NoSuchPeer => "Internal error: No such peer",
   }
}
