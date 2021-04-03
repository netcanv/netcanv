// the netcanv matchmaker server.
// keeps track of open rooms and exchanges addresses between hosts and their clients

use std::collections::{HashMap};
use std::error;
use std::net::{AddrParseError, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, Weak};

use thiserror::Error;

use netcanv_protocol::matchmaker::*;

const PORT: u16 = 62137;

const MAX_ROOM_ID: u32 = 9999;

#[derive(Clone, Debug)]
struct Room {
    host: Arc<TcpStream>,
    clients: Vec<Weak<TcpStream>>,
    id: u32,
}

struct Matchmaker {
    rooms: HashMap<u32, Room>,
    host_rooms: HashMap<SocketAddr, u32>,
    relay_clients: HashMap<SocketAddr, u32>, // mapping address → room ID
}

#[derive(Debug, Error)]
enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unrecognized or unimplemented packet")]
    InvalidPacket,
    #[error("Invalid packet (bad encoding)")]
    Deserialize,
    #[error("Serialization error: {0}")]
    Serialize(#[from] bincode::Error),
    #[error("Invalid address: {0}")]
    InvalidAddr(#[from] AddrParseError),
}

impl Matchmaker {

    fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            host_rooms: HashMap::new(),
            relay_clients: HashMap::new(),
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
        eprintln!("- sending packet {} -> {:?}", stream.peer_addr()?, packet);
        bincode::serialize_into(stream, &packet)?;
        Ok(())
    }

    fn send_error(stream: &TcpStream, error: &str) -> Result<(), Error> {
        Self::send_packet(stream, error_packet(error))
    }

    fn host(mm: Arc<Mutex<Self>>, peer_addr: SocketAddr, stream: Arc<TcpStream>) -> Result<(), Error> {
        let mut mm = mm.lock().unwrap();
        match mm.find_free_room_id() {
            Some(room_id) => {
                let room = Room {
                    host: stream.clone(),
                    clients: Vec::new(),
                    id: room_id
                };
                {
                    mm.rooms.insert(room_id, room);
                    mm.host_rooms.insert(peer_addr, room_id);
                }
                drop(mm);
                Self::send_packet(&stream, Packet::RoomId(room_id))?;
            },
            None => Self::send_error(&stream, "Could not find any more free rooms. Try again")?,
        }
        Ok(())
    }

    fn join(mm: Arc<Mutex<Self>>, stream: &TcpStream, room_id: u32) -> Result<(), Error> {
        let mm = mm.lock().unwrap();
        let room = match mm.rooms.get(&room_id) {
            Some(room) => room,
            None => {
                Self::send_error(stream,
                    "No room found with the given ID. Check whether you spelled the ID correctly")?;
                return Ok(());
            },
        };
        let client_addr = stream.peer_addr()?;
        let host_addr = room.host.peer_addr()?;
        Self::send_packet(&room.host, Packet::ClientAddress(client_addr))?;
        Self::send_packet(stream, Packet::HostAddress(host_addr))
    }

    fn add_relay(mm: Arc<Mutex<Self>>, stream: Arc<TcpStream>, host_addr: Option<SocketAddr>) -> Result<(), Error> {
        let peer_addr = stream.peer_addr().unwrap();
        eprintln!("- relay requested from {}", peer_addr);

        let host_addr: SocketAddr = host_addr.unwrap_or(peer_addr);
        {
            let mut mm = mm.lock().unwrap();
            let room_id: u32;
            match mm.host_rooms.get(&host_addr) {
                Some(id) => room_id = *id,
                None => {
                    Self::send_error(&stream, "The host seems to have disconnected")?;
                    return Ok(());
                },
            }
            mm.relay_clients.insert(peer_addr, room_id);
            mm.rooms.get_mut(&room_id).unwrap().clients.push(Arc::downgrade(&stream));
        }

        Ok(())
    }

    fn relay(mm: Arc<Mutex<Self>>, stream: &Arc<TcpStream>, to: Option<SocketAddr>, data: &[u8]) -> Result<(), Error> {
        // XXX: this can bottleneck the server if there are many relays running at the same time
        // because the mutex is locked for the entire duration of the server relaying packets!!!
        let addr = stream.peer_addr().unwrap();
        {
            let mut mm = mm.lock().unwrap();
            let room_id =
                match mm.relay_clients.get(&addr) {
                    Some(id) => *id,
                    None => {
                        Self::send_error(stream, "Only relay clients may send Relay packets")?;
                        return Ok(())
                    },
                };
            match mm.rooms.get_mut(&room_id) {
                Some(room) => {
                    let mut nclients = 0;
                    room.clients.retain(|client| client.upgrade().is_some());
                    for client in &room.clients {
                        let client = &client.upgrade().unwrap();
                        if !Arc::ptr_eq(client, stream) {
                            if let Some(addr) = to {
                                if client.peer_addr()? != addr {
                                    continue;
                                }
                            }
                            Self::send_packet(client, Packet::Relayed(addr, Vec::from(data)))?;
                            nclients += 1;
                        }
                    }
                    eprintln!("- relayed to {} clients", nclients);
                },
                None => {
                    Self::send_error(stream, "The host seems to have disconnected")?;
                    return Ok(())
                },
            };
        }

        Ok(())
    }

    fn incoming_packet(
        mm: Arc<Mutex<Self>>,
        peer_addr: SocketAddr,
        stream: Arc<TcpStream>,
        packet: Packet
    ) -> Result<(), Error> {
        eprintln!("- incoming packet: {:?}", packet);
        match packet {
            Packet::Host => Self::host(mm, peer_addr, stream),
            Packet::GetHost(room_id) => Self::join(mm, &stream, room_id),
            Packet::RequestRelay(host_addr) => {
                Self::add_relay(mm, stream, host_addr)
            },
            Packet::Relay(to, data) => Self::relay(mm, &stream, to, &data),
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
        self.relay_clients.remove(&addr);
    }

    fn start_client_thread(mm: Arc<Mutex<Self>>, stream: TcpStream) -> Result<(), Error> {
        let peer_addr = stream.peer_addr()?;
        let stream = Arc::new(stream);
        eprintln!("* mornin' mr. {}", peer_addr);
        let _ = std::thread::spawn(move || {
            loop {
                let mut buf = [0; 1];
                if let Ok(n) = stream.peek(&mut buf) {
                    if n == 0 {
                        mm.lock().unwrap().disconnect(peer_addr);
                        break
                    }
                }
                let _ = bincode::deserialize_from(&*stream) // what
                    .map_err(|_| Error::Deserialize)
                    .and_then(|decoded| {
                        Self::incoming_packet(mm.clone(), peer_addr, stream.clone(), decoded)
                    })
                    .or_else(|error| -> Result<_, ()> {
                        eprintln!("! error/packet decode from {}: {}", peer_addr, error);
                        Ok(())
                    });
            }
            eprintln!("* bye bye mr. {} it was nice to see ya", peer_addr);
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

