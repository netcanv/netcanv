// the netcanv matchmaker server.
// keeps track of open rooms and exchanges addresses between hosts and their clients

use std::collections::{HashMap};
use std::error;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

use thiserror::Error;

use netcanv_protocol::matchmaker::*;

const PORT: u16 = 62137;

const MAX_ROOM_ID: u32 = 9999;

#[derive(Clone, Debug)]
struct Host {
    stream: Arc<TcpStream>,
    room_id: u32,
}

struct Matchmaker {
    rooms: HashMap<u32, Host>,
    host_rooms: HashMap<SocketAddr, u32>
}

#[derive(Debug, Error)]
enum Error {
    #[error("An I/O error occured: {0}")]
    Io(#[from] std::io::Error),
    #[error("Couldn't receive packet")]
    Recv,
    #[error("Unrecognized or unimplemented packet")]
    InvalidPacket,
    #[error("Invalid packet (bad encoding)")]
    Deserialize,
    #[error("Serialization error: {0}")]
    Serialize(#[from] bincode::Error),
}

impl Matchmaker {

    fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            host_rooms: HashMap::new(),
        }
    }

    fn find_free_room_id(&self) -> Option<u32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for _ in 1..50 {
            let id = rng.gen_range(0..=MAX_ROOM_ID);
            if !self.rooms.contains_key(&id) {
                return Some(id)
            }
        }
        None
    }

    fn send_packet(stream: &TcpStream, packet: Packet) -> Result<(), Error> {
        bincode::serialize_into(stream, &packet)?;
        Ok(())
    }

    fn send_error(stream: &TcpStream, error: &str) -> Result<(), Error> {
        Self::send_packet(stream, error_packet(error))
    }

    fn host(mm: Arc<Mutex<Self>>, stream: Arc<TcpStream>) -> Result<(), Error> {
        match mm.lock().unwrap().find_free_room_id() {
            Some(room_id) => {
                let host = Host { stream: stream.clone(), room_id };
                {
                    let mut mm = mm.lock().unwrap();
                    let host_addr = host.stream.peer_addr().unwrap();
                    mm.rooms.insert(room_id, host);
                    mm.host_rooms.insert(host_addr, room_id);
                }
                Self::send_packet(&stream, Packet::RoomId(room_id))?;
            },
            None => Self::send_error(&stream, "Could not find any more free rooms. Try again")?,
        }
        Ok(())
    }

    fn join(mm: Arc<Mutex<Self>>, stream: &TcpStream, room_id: u32) -> Result<(), Error> {
        let mm = mm.lock().unwrap();
        let host = match mm.rooms.get(&room_id) {
            Some(host) => host,
            None => {
                Self::send_error(stream,
                    "No room found with the given ID. Check whether you spelled the ID correctly")?;
                return Ok(());
            },
        };
        let client_addr = stream.peer_addr()?;
        let host_addr = host.stream.peer_addr()?;
        Self::send_packet(&host.stream, Packet::ClientAddress(client_addr.to_string()))?;
        Self::send_packet(stream, Packet::HostAddress(host_addr.to_string()))
    }

    fn incoming_packet(mm: Arc<Mutex<Self>>, stream: Arc<TcpStream>, packet: Packet) -> Result<(), Error> {
        match packet {
            Packet::Host => Self::host(mm, stream),
            Packet::GetHost(room_id) => Self::join(mm, &stream, room_id),
            _ => {
                eprintln!("! error/invalid packet: {:?}", packet);
                Err(Error::InvalidPacket)
            },
        }
    }

    fn disconnect(&mut self, addr: SocketAddr) {
        if let Some(room_id) = self.host_rooms.remove(&addr) {
            self.rooms.remove(&room_id);
        }
    }

    fn start_client_thread(mm: Arc<Mutex<Self>>, stream: TcpStream) -> Result<(), Error> {
        let peer_addr = stream.peer_addr()?;
        let stream = Arc::new(stream);
        eprintln!("* connected: {}", peer_addr);
        let _ = std::thread::spawn(move || {
            loop {
                let mut buf = [0; 1];
                if let Ok(n) = stream.peek(&mut buf) {
                    if n == 0 {
                        let addr = stream.peer_addr().unwrap();
                        mm.lock().unwrap().disconnect(addr);
                        break
                    }
                }
                bincode::deserialize_from(&*stream) // what
                    .map_err(|_| Error::Deserialize)
                    .and_then(|decoded| {
                        Self::incoming_packet(mm.clone(), stream.clone(), decoded)
                    })
                    .or_else(|error| -> Result<_, ()> {
                        eprintln!("! error/packet decode from {}: {}", peer_addr, error);
                        Ok(())
                    })
                    .unwrap();
            }
        });
        Ok(())
    }

}

fn main() -> Result<(), Box<dyn error::Error>> {
    eprintln!("NetCanv Matchmaker: starting on port {}", PORT);

    let localhost = SocketAddr::from(([127, 0, 0, 1], PORT));
    let listener = TcpListener::bind(localhost)?;

    let state = Arc::new(Mutex::new(Matchmaker::new()));

    eprintln!("Listening for incoming connections");

    for connection in listener.incoming() {
        connection
            .map_err(|error| Error::from(error))
            .and_then(|stream| Matchmaker::start_client_thread(state.clone(), stream))
            .or_else(|error| -> Result<_, ()> {
                eprintln!("! error/connect: {}", error);
                Ok(())
            })
            .unwrap(); // silence, compiler
    }

    Ok(())
}

