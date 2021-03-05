// matchmaker packets

use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
    // request from the host to the matchmaker for a free ID
    Host,
    // response from the matchmaker to the host containing the ID
    RoomId(u32),
    // request from a client to join a room with the given ID
    GetHost(u32),
    // response from the matchmaker to the client containing the host's IP address and port
    HostAddress(String),
    // notification from the matchmaker to the host with a connecting client's IP address and port
    ClientAddress(String),

    // an error occured
    Error(String),
}

// fast way to create an error packet
pub fn error_packet(message: &str) -> Packet {
    Packet::Error(message.to_string())
}
