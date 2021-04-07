// matchmaker packets

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
    //
    // initial hosting procedure
    //

    // request from the host to the matchmaker for a free ID
    Host,
    // response from the matchmaker to the host containing the ID
    RoomId(u32),
    // request from a client to join a room with the given ID
    GetHost(u32),
    // response from the matchmaker to the client containing the host's IP address and port
    HostAddress(SocketAddr),
    // notification from the matchmaker to the host with a connecting client's IP address and port
    ClientAddress(SocketAddr),

    //
    // packet relay
    //

    // request for the matchmaker to serve as a packet relay for clients incapable of making direct P2P connections
    RequestRelay(Option<SocketAddr>),

    // payload to be relayed. the first argument is an optional target to relay to
    Relay(Option<SocketAddr>, Vec<u8>),
    // relayed payload.
    // in version 2 of the protocol there's a special case for this packet, where if the Vec is empty, the packet is
    // a response to the RequestRelay packet.
    Relayed(SocketAddr, Vec<u8>),

    // a relay client has disconnected. sent out to relay clients because they can't normally tell if one of their
    // peers has disconnected
    Disconnected(SocketAddr),

    //
    // other
    //

    // an error occured
    Error(String),
}

// fast way to create an error packet
pub fn error_packet(message: &str) -> Packet {
    Packet::Error(message.to_string())
}
