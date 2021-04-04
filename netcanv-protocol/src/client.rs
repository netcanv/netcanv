// client (p2p) packets

use serde::{Serialize, Deserialize};

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
    //
    // introduction protocol
    //

    // introduction to other clients. the string contains the nickname
    Hello(String),

    // response from the other clients with their nicknames
    HiThere(String),

    // image data sent to a client by the host when it first joins
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

