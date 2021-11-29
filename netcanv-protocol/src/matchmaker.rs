// matchmaker packets

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

/// The default matchmaker port.
pub const DEFAULT_PORT: u16 = 62137;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
   // ---
   // Initial hosting procedure
   // ---
   /// Request from the host to the matchmaker for a free room ID.
   Host,
   /// Response from the matchmaker to the host containing the room ID, and the peer ID inside the
   /// room.
   RoomCreated(RoomId, PeerId),
   /// Request sent from a client, to join a room with the given ID.
   Join(RoomId),
   /// Response from the matchmaker to the client containing the host's peer ID.
   HostId(PeerId),

   // ---
   // Packet relay
   // ---
   /// Payload to be relayed. The first argument is the target to relay to.
   ///
   /// If the target is [`PeerID::BROADCAST`], the packet will be sent out to all the peers in
   /// the room.
   Relay(PeerId, Vec<u8>),
   /// Payload relayed from another peer.
   Relayed(PeerId, Vec<u8>),

   /// A peer has left the room.
   Disconnected(PeerId),

   // ---
   // Other
   // ---
   /// An error occured.
   Error(Error),
}

/// The unique ID of a room.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct RoomId(pub [u8; Self::LEN]);

impl RoomId {
   /// The length of a room ID.
   pub const LEN: usize = 6;
}

impl TryFrom<&str> for RoomId {
   type Error = RoomIdError;

   fn try_from(value: &str) -> Result<Self, Self::Error> {
      if value.len() != Self::LEN {
         Err(RoomIdError(()))
      } else {
         let mut bytes = [0u8; Self::LEN];
         for (i, byte) in value.bytes().enumerate() {
            bytes[i] = byte;
         }
         Ok(RoomId(bytes))
      }
   }
}

impl Display for RoomId {
   fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
      // Should not panic, as room ID is always
      match std::str::from_utf8(&self.0) {
         Ok(s) => write!(f, "{}", s),
         Err(_) => write!(f, "<invalid UTF-8>"),
      }
   }
}

/// An error returned in case the room ID is not made up of characters.
#[derive(Debug)]
pub struct RoomIdError(());

impl std::error::Error for RoomIdError {}

impl Display for RoomIdError {
   fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
      write!(f, "room ID must be 6 characters long")
   }
}

/// The inner type for storing a peer ID.
type PeerIdInner = u64;

/// The unique ID of a peer.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct PeerId(pub PeerIdInner);

impl PeerId {
   /// The broadcast ID. Any occurrence of this signifies that a message should be broadcast
   /// to all clients in a room.
   pub const BROADCAST: Self = Self(0);

   /// The first peer.
   pub const FIRST_PEER: PeerIdInner = 1;

   /// The last peer.
   pub const LAST_PEER: PeerIdInner = PeerIdInner::MAX;

   /// Returns whether the peer ID is the one used for broadcasting messages.
   pub fn is_broadcast(self) -> bool {
      self == Self::BROADCAST
   }
}

/// An error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Error {
   /// No more free room IDs available.
   NoFreeRooms,
   /// No more free peer IDs available.
   ///
   /// Like, this shouldn't happen. If it happens, well…
   ///
   /// The peer ID is stored in a `u64`. Good luck exhausting that.
   NoFreePeerIDs,
   /// The room with the given ID does not exist.
   RoomDoesNotExist,
   /// The peer with the given ID doesn't seem to be connected.
   NoSuchPeer,
}
