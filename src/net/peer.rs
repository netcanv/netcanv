use std::net::SocketAddr;
use std::thread;

use crossbeam_channel::{Receiver, Sender};
use laminar::{Socket, SocketEvent};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use thiserror::Error;

use netcanv_protocol::matchmaker as mm;

#[derive(Debug, Error)]
pub enum PeerError {
    // uncontrollable errors
    #[error("A network error has occured: {0}")]
    Network(#[from] laminar::ErrorKind),
    #[error("An error occured while serializing packets: {0}")]
    Serialize(#[from] bincode::Error),
    #[error("An internal multithreading error has occured while receiving packets: {0}")]
    Recv(#[from] crossbeam_channel::RecvError),
    #[error("The packet could not be sent due to a multithreading error")]
    Send,

    // controlled errors
    #[error("The message recipient has lost connection")]
    Disconnect,
    #[error("An unexpected packet was received from the sender")]
    InvalidPacket,
    #[error("{0}")]
    ErrorPacket(String),
}

pub enum PeerKind {
    Host,
    Client,
}

enum Message {
    // thread finished with a room ID
    RoomId(u32),
    // thread finished with a host address
    HostAddress(SocketAddr),
}

struct NetworkThread {
    socket: Socket,
    to: Option<Sender<Message>>,
    from: Option<Receiver<Message>>,
    join_handle: thread::JoinHandle<Result<Message, PeerError>>,
}

pub struct Peer {
    kind: PeerKind,
    is_self: bool,
    thread: NetworkThread,
}

// don't do this
macro_rules! expect_packet_for {
    ($err_patt:pat, $err_let:ident, $name:ident) => {
        macro_rules! $name {
            ($rx:expr, $patt:pat, $then:block) => {
                match Peer::recv_packet($rx)? {
                    $patt => $then,
                    $err_patt => return Err(PeerError::ErrorPacket($err_let)),
                    _ => return Err(PeerError::InvalidPacket),
                }
            };
        }
    };
}

expect_packet_for!(mm::Packet::Error(error), error, expect_mm_packet);

impl Peer {

    fn recv_packet<P>(rx: &Receiver<SocketEvent>) -> Result<P, PeerError>
        where P: DeserializeOwned,
    {
        loop {
            let event = rx.recv()?;
            match event {
                SocketEvent::Packet(packet) => {
                    let payload = packet.payload();
                    let deserialized = bincode::deserialize::<P>(payload)?;
                    return Ok(deserialized)
                },
                SocketEvent::Connect(_) => (),
                SocketEvent::Timeout(_) | SocketEvent::Disconnect(_) => {
                    return Err(PeerError::Disconnect)
                },
            }
        }
    }

    fn send_packet<P>(tx: &Sender<laminar::Packet>, addr: SocketAddr, packet: P) -> Result<(), PeerError>
        where P: Serialize,
    {
        let serialized = bincode::serialize(&packet)?;
        let packet = laminar::Packet::reliable_ordered(addr, serialized, None);
        tx.send(packet).map_err(|_| PeerError::Send)
    }

    fn make_match(matchmaker_addr: SocketAddr, room_id: Option<u32>) -> Result<NetworkThread, PeerError> {
        let matchmaker = Socket::bind(matchmaker_addr)?;
        let (tx, rx) = (matchmaker.get_packet_sender(), matchmaker.get_event_receiver());

        let func = move || -> Result<Message, PeerError> {
            if let Some(room_id) = room_id {
                Self::send_packet(&tx, matchmaker_addr, mm::Packet::GetHost(room_id))?;
                expect_mm_packet!(&rx, mm::Packet::HostAddress(host_addr_str), {
                    let host_addr: SocketAddr = host_addr_str.parse().map_err(|_| PeerError::InvalidPacket)?;
                    return Ok(Message::HostAddress(host_addr));
                });
            } else {
                Self::send_packet(&tx, matchmaker_addr, mm::Packet::Host)?;
                expect_mm_packet!(&rx, mm::Packet::RoomId(room_id), {
                    return Ok(Message::RoomId(room_id));
                });
            }
        };

        Ok(NetworkThread {
            socket: matchmaker,
            to: None,
            from: None,
            join_handle: thread::spawn(func),
        })
    }

    pub fn host_room(matchmaker_addr: SocketAddr) -> Result<Self, PeerError> {
        Ok(Self {
            kind: PeerKind::Host,
            is_self: true,
            thread: Self::make_match(matchmaker_addr, None)?,
        })
    }

    pub fn join_room(matchmaker_addr: SocketAddr, room_id: u32) -> Result<Self, PeerError> {
        Ok(Self {
            kind: PeerKind::Client,
            is_self: true,
            thread: Self::make_match(matchmaker_addr, Some(room_id))?,
        })
    }

}
