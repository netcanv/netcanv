// client (p2p) packets

use serde::{Deserialize, Serialize};

// the version constant. increased by 100 every minor client version, and by 10000 every major version.
// eg. 200 is 0.2.0, 10000 is 1.0.0, 10203 is 1.2.3.
// if two versions' hundreds places differ, the versions are incompatible.
pub const PROTOCOL_VERSION: u32 = 200;

pub fn versions_compatible(v1: u32, v2: u32) -> bool {
    v1 / 100 == v2 / 100
}

pub fn compatible_with(v: u32) -> bool {
    versions_compatible(PROTOCOL_VERSION, v)
}

// stroke packet information
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct StrokePoint {
    // 29.3 fixed-point coordinates of the point
    pub x: i32,
    pub y: i32,
    // hex-encoded color
    // a value of 0 is special and means eraser mode
    pub color: u32,
    // 15.1 fixed-point brush size
    pub brush_size: i16,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
    /*--
     * VERSION 0.1.0 (no version packet)
     */
    //
    // introduction protocol
    //

    // introduction to other clients. the string contains the nickname
    Hello(String),

    // response from the other clients with their nicknames
    HiThere(String),

    // image data sent to a client by the host when it first joins
    #[deprecated(since = "0.2.0", note = "use Chunks instead; will be removed in 0.3.0")]
    CanvasData((i32, i32), Vec<u8>),

    //
    // painting
    // --------
    // these packets are sent 20 times per second
    //

    // cursor packet containing fixed-point 29.3 coordinates and a fixed-point 31.1 brush size
    Cursor(i32, i32, i16),

    // a paint stroke
    Stroke(Vec<StrokePoint>),

    /*--
     * VERSION 0.2.0 (protocol 200)
     */
    // version packet. this is sent as part of a response to Hello
    Version(u32),

    // sent by the host to a client upon connection
    ChunkPositions(Vec<(i32, i32)>),

    // request from the client to download chunks
    GetChunks(Vec<(i32, i32)>),

    // response from the other peer with the chunks encoded as PNG images.
    Chunks(Vec<((i32, i32), Vec<u8>)>),
}

/// converts a float to a fixed-point 29.3
pub fn to_fixed29p3(x: f32) -> i32 {
    (x * 8.0).round() as i32
}

/// converts a float to a fixed-point 15.1
pub fn to_fixed15p1(x: f32) -> i16 {
    (x * 2.0).round() as i16
}

/// converts a fixed-point 29.3 to a float
pub fn from_fixed29p3(x: i32) -> f32 {
    x as f32 / 8.0
}

/// converts a fixed-point 15.1 to a float
pub fn from_fixed15p1(x: i16) -> f32 {
    x as f32 / 2.0
}
