///! Client communication packets.
use serde::{Deserialize, Serialize};

/// The version constant. Increased by 100 every minor client version, and by 10000 every major
/// version. eg. 200 is 0.2.0, 10000 is 1.0.0, 10203 is 1.2.3.
/// If two versions' hundreds places differ, the versions are incompatible.
pub const PROTOCOL_VERSION: u32 = 300;

pub fn versions_compatible(v1: u32, v2: u32) -> bool {
   v1 / 100 == v2 / 100
}

pub fn compatible_with(v: u32) -> bool {
   versions_compatible(PROTOCOL_VERSION, v)
}

/// A client communication packet.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
   // ---
   // VERSION 0.1.0 (no version packet)
   // ---

   //
   // Introduction protocol
   //
   /// Introduction to other clients. The string contains the nickname.
   Hello(String),

   /// Response from the other clients with their nicknames.
   HiThere(String),

   /// Reserved, formerly `CanvasData`.
   Reserved1,

   //
   // Tools
   // --------
   // These packets are sent 20 times per second, and are used for exchanging tool-specific
   // information. Each tool can define its own packet for communication.
   //
   /// Carries a payload with a tool-specific packet.
   Tool(String, Vec<u8>),

   /// Notifies that a different tool was selected.
   SelectTool(String),

   // ---
   // VERSION 0.2.0 (protocol 200)
   // ---
   /// Version packet. This is sent as part of a response to Hello.
   Version(u32),

   /// Sent by the host to a client upon connection.
   ChunkPositions(Vec<(i32, i32)>),

   /// Request from the client to download chunks.
   GetChunks(Vec<(i32, i32)>),

   /// Response from the other peer with the chunks encoded as PNG images.
   Chunks(Vec<((i32, i32), Vec<u8>)>),
   /* ---
    * VERSION 0.3.0 (protocol 300)
    * ---
    * No changes in available packets, but chunks may now be sent in webp which makes hosts using
    * this version incompatible with older clients.
    *
    * ---
    * VERSION 0.4.0 (protocol 400)
    * ---
    * Cursor and Stroke packets were removed in favor of the generic Tool packet.
    * Each tool is responsible for decoding its own packets now.
    */
}
