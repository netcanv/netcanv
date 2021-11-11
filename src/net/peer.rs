use std::any::Any;
use std::collections::HashMap;
use std::mem;
use std::net::SocketAddr;
use std::sync::Arc;

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
   Joined(String, SocketAddr),
   /// Another peer has left the room.
   Left(String),
   /// A tool packet was received from an address.
   Tool(SocketAddr, String, Vec<u8>),
   /// The host sent us the chunk positions for the room.
   ChunkPositions(Vec<(i32, i32)>),
   /// Somebody requested chunk positions from the host.
   GetChunks(SocketAddr, Vec<(i32, i32)>),
   /// Somebody sent us chunk image data.
   Chunks(Vec<((i32, i32), Vec<u8>)>),
}

/// The state of a Peer connection.
#[derive(Debug)]
enum State {
   // No connection has been established yet. We're waiting on the socket subsystem to give us a socket.
   WaitingForMatchmaker { token: ConnectionToken },
   // We're connected to the matchmaker, but haven't obtained the other person's connection
   // details yet.
   ConnectedToMatchmaker,
   // We're waiting for the matchmaker to respond on relaying our packets.
   WaitingForRelay,
   // We're hosting a room.
   HostingRoomRelayed,
   // We're connected to a host.
   InRoomRelayed,
}

/// A connection to the matchmaker.
pub struct Peer {
   token: PeerToken,
   state: State,
   matchmaker_socket: Option<Socket<mm::Packet>>,

   is_host: bool,

   nickname: String,
   room_id: Option<u32>,
   host: Option<SocketAddr>,
   mates: HashMap<SocketAddr, Mate>,
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
         mates: HashMap::new(),
         host: None,
      })
   }

   /// Join an existing room on the given matchmaker.
   pub fn join(
      socksys: &Arc<SocketSystem<mm::Packet>>,
      nickname: &str,
      matchmaker_address: &str,
      room_id: u32,
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
         State::ConnectedToMatchmaker
         | State::WaitingForRelay
         | State::HostingRoomRelayed
         | State::InRoomRelayed => self.matchmaker_socket.as_ref().unwrap().send(packet),
         _ => anyhow::bail!("cannot send packet: not connected to the matchmaker"),
      }
      Ok(())
   }

   /// Sends a client packet to the peer with the given address, or if no address is provided, to
   /// everyone.
   fn send_to_client(&self, to: Option<SocketAddr>, packet: cl::Packet) -> anyhow::Result<()> {
      // TODO: p2p communication without the relay
      match &self.state {
         State::HostingRoomRelayed | State::InRoomRelayed => {
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
      self.matchmaker_socket = Some(socket);
      match state {
         State::WaitingForMatchmaker { .. } => self.send_to_matchmaker(if self.is_host {
            mm::Packet::Host
         } else {
            mm::Packet::GetHost(self.room_id.unwrap())
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
            State::ConnectedToMatchmaker
            | State::WaitingForRelay
            | State::HostingRoomRelayed
            | State::InRoomRelayed
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
         mm::Packet::RoomId(room_id) => {
            eprintln!("got free room ID: {}", room_id);
            self.room_id = Some(room_id);
            self.setup_relay()?;
         }
         mm::Packet::HostAddress(address) => {
            eprintln!("got host address: {}", address);
            self.host = Some(address);
            self.setup_relay()?;
         }
         mm::Packet::ClientAddress(_address) => (),
         mm::Packet::Relayed(_, payload) if payload.len() == 0 => {
            eprintln!("got successful packet relay connection");
            self.state = if self.is_host {
               State::HostingRoomRelayed
            } else {
               State::InRoomRelayed
            };
            self.say_hello()?;
            bus::push(Connected { peer: self.token });
         }
         mm::Packet::Relayed(author, payload) => {
            let client_packet: cl::Packet = bincode::deserialize(&payload)?;
            self.client_packet(author, client_packet)?;
         }
         mm::Packet::Disconnected(address) => {
            if let Some(mate) = self.mates.remove(&address) {
               self.send_message(MessageKind::Left(mate.nickname));
            }
         }
         mm::Packet::Error(message) => anyhow::bail!(message),
         other => anyhow::bail!("unexpected matchmaker packet: {:?}", other),
      }
      Ok(())
   }

   /// Sets up the packet relay.
   fn setup_relay(&mut self) -> anyhow::Result<()> {
      let relay_target = (!self.is_host).then(|| self.host.unwrap());
      eprintln!("requesting relay to host {:?}", relay_target);
      self.send_to_matchmaker(mm::Packet::RequestRelay(relay_target))?;
      self.state = State::WaitingForRelay;
      Ok(())
   }

   /// Says hello to other peers in the room.
   fn say_hello(&self) -> anyhow::Result<()> {
      self.send_to_client(None, cl::Packet::Hello(self.nickname.clone()))
   }

   /// Decodes a client packet.
   fn client_packet(&mut self, author: SocketAddr, packet: cl::Packet) -> anyhow::Result<()> {
      match packet {
         // -----
         // 0.1.0
         // -----
         cl::Packet::Hello(nickname) => {
            eprintln!("{} ({}) joined", nickname, author);
            self.send_to_client(Some(author), cl::Packet::HiThere(self.nickname.clone()))?;
            self.send_to_client(Some(author), cl::Packet::Version(cl::PROTOCOL_VERSION))?;
            self.add_mate(author, nickname.clone());
            self.send_message(MessageKind::Joined(nickname, author));
         }
         cl::Packet::HiThere(nickname) => {
            eprintln!("{} ({}) is in the room", nickname, author);
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
         cl::Packet::SelectTool(name) => todo!(),
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
   fn add_mate(&mut self, addr: SocketAddr, nickname: String) {
      self.mates.insert(
         addr,
         Mate {
            nickname,
            tool: None,
         },
      );
   }

   /// Sends a chunk positions packet.
   pub fn send_chunk_positions(
      &self,
      to: SocketAddr,
      positions: Vec<(i32, i32)>,
   ) -> anyhow::Result<()> {
      self.send_to_client(Some(to), cl::Packet::ChunkPositions(positions))
   }

   /// Requests chunk data from the host.
   pub fn download_chunks(&self, positions: Vec<(i32, i32)>) -> anyhow::Result<()> {
      assert!(self.host.is_some(), "only non-hosts can download chunks");
      eprintln!("downloading {} chunks from the host", positions.len());
      self.send_to_client(self.host, cl::Packet::GetChunks(positions))
   }

   /// Sends chunks to the given peer.
   pub fn send_chunks(
      &self,
      to: SocketAddr,
      chunks: Vec<((i32, i32), Vec<u8>)>,
   ) -> anyhow::Result<()> {
      self.send_to_client(Some(to), cl::Packet::Chunks(chunks))
   }

   /// Sends a tool-specific packet.
   pub fn send_tool(&self, name: String, payload: Vec<u8>) -> anyhow::Result<()> {
      self.send_to_client(None, cl::Packet::Tool(name, payload))
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
   pub fn room_id(&self) -> Option<u32> {
      self.room_id
   }

   /// Returns the list of peers connected to the same room.
   pub fn mates(&self) -> &HashMap<SocketAddr, Mate> {
      &self.mates
   }
}

/// Another person in the same room.
pub struct Mate {
   pub nickname: String,
   pub tool: Option<(String, Box<dyn Any>)>,
}
