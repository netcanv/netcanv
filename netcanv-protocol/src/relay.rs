//! Relay packaets.

use std::{
   fmt::{self, Display, Formatter},
   str::FromStr,
};

use serde::{Deserialize, Serialize};

#[cfg(feature = "i18n")]
use netcanv_i18n::Formatted;

/// The default relay port.
pub const DEFAULT_PORT: u16 = 62137;

/// The version of the protocol.
///
/// This is sent by the server upon connecting, before any packets.
// The version is incremented whenever breaking changes are introduced in the protocol.
pub const PROTOCOL_VERSION: u32 = 1;

/// The maximum length of a serialized packet. If a packet is larger than this amount, the
/// connection shall be closed.
// 4 MiB for now, should be plenty. Chunk packets are never larger than 128 KiB, and clipboard
// images are downscaled to max 1024x1024. A 1024x1024 PNG of RGB noise is about 2 MiB.
pub const MAX_PACKET_SIZE: u32 = 4 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
   // ---
   // Initial hosting procedure
   // ---
   /// Request from the host to the relay for a free room ID.
   Host,
   /// Response from the relay to the host containing the room ID, and the peer ID inside the
   /// room.
   RoomCreated(RoomId, PeerId),
   /// Request sent from a client, to join a room with the given ID.
   Join(RoomId),
   /// Response from the relay to the client containing the client's peer ID and the host's
   /// peer ID.
   Joined { peer_id: PeerId, host_id: PeerId },
   /// Message from the relay that the host has disconnected, and that the host role now
   /// belongs to the peer with the given ID.
   HostTransfer(PeerId),

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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct RoomId(pub [u8; Self::LEN]);

impl RoomId {
   /// The length of a room ID.
   pub const LEN: usize = 6;
}

impl FromStr for RoomId {
   type Err = RoomIdError;

   fn from_str(value: &str) -> Result<Self, Self::Err> {
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
      match std::str::from_utf8(&self.0) {
         Ok(s) => write!(f, "{}", s),
         Err(_) => write!(f, "<invalid UTF-8>"),
      }
   }
}

impl fmt::Debug for RoomId {
   fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
      write!(f, "r:{}", self)
   }
}

/// An error returned in case the room ID is not made up of 6 characters.
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
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

impl Display for PeerId {
   fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
      write!(f, "p:{:016x}", self.0)
   }
}

impl fmt::Debug for PeerId {
   fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self)
   }
}

#[cfg(feature = "i18n")]
impl From<PeerId> for netcanv_i18n::FormatArg<'_> {
   fn from(value: PeerId) -> Self {
      Self::Unsigned(value.0)
   }
}

/// An error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "i18n", derive(netcanv_i18n::TranslateEnum))]
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
   NoSuchPeer { address: PeerId },
}
