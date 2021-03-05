// the netcanv matchmaker server.
// keeps track of open rooms and exchanges addresses between hosts and their clients

use std::collections::{HashMap};
use std::error;
use std::net::SocketAddr;

use laminar::{Socket, SocketEvent};
use thiserror::Error;

use netcanv::net::txqueues::{SendQueue, SendError};

mod packet;

pub use packet::*;

const PORT: u16 = 62137;

const MAX_ROOM_ID: u32 = 9999;

#[derive(Copy, Clone, Debug)]
struct Host {
    addr: SocketAddr,
    room_id: u32,
}

struct Matchmaker {
    hosts: HashMap<SocketAddr, Host>,
    rooms: HashMap<u32, Host>,
    send_queue: SendQueue<Packet>,
}

#[derive(Debug, Error)]
enum Error {
    #[error("Couldn't receive packet")]
    Recv,
    #[error("Couldn't send packet")]
    Send(#[from] SendError),
    #[error("Unrecognized or unimplemented packet")]
    InvalidPacket,
    #[error("Invalid packet (bad encoding)")]
    Deserialize,
}

impl Matchmaker {

    fn new() -> Self {
        Self {
            hosts: HashMap::new(),
            rooms: HashMap::new(),
            send_queue: SendQueue::new(),
        }
    }

    fn enqueue_packet(&mut self, dest_addr: SocketAddr, packet: Packet) {
        self.send_queue.enqueue(dest_addr, packet);
    }

    fn enqueue_error(&mut self, dest_addr: SocketAddr, message: &str) {
        self.enqueue_packet(dest_addr, error_packet(message));
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

    fn host(&mut self, host_addr: SocketAddr) {
        match self.find_free_room_id() {
            Some(room_id) => {
                let host = Host { addr: host_addr, room_id };
                self.hosts.insert(host_addr, host.clone());
                self.rooms.insert(room_id, host);
                self.enqueue_packet(host_addr, Packet::RoomId(room_id));
            },
            None => self.enqueue_error(host_addr, "Could not find any more free rooms. Try again"),
        }
    }

    fn join(&mut self, client_addr: SocketAddr, room_id: u32) {
        let host = match self.rooms.get(&room_id) {
            Some(host) => host,
            None => {
                self.enqueue_error(client_addr, "No room found with the given ID. Check spelling of the ID");
                return;
            },
        }.clone();
        self.enqueue_packet(host.addr, Packet::ClientAddress(client_addr.to_string()));
        self.enqueue_packet(client_addr, Packet::HostAddress(host.addr.to_string()));
    }

    fn incoming_packet(&mut self, addr: SocketAddr, packet: Packet) -> Result<(), Error> {
        match packet {
            Packet::Host => self.host(addr),
            Packet::GetHost(room_id) => self.join(addr, room_id),
            _ => {
                eprintln!("! error/invalid packet: {:?}", packet);
                return Err(Error::InvalidPacket);
            },
        }
        Ok(())
    }

    fn disconnect(&mut self, addr: SocketAddr) {
        if let Some(host) = self.hosts.remove(&addr) {
            self.rooms.remove(&host.room_id);
        }
    }

    fn incoming_event(&mut self, event: SocketEvent) -> Result<(), Error> {
        match event {
            SocketEvent::Packet(packet) => {
                bincode::deserialize(packet.payload())
                    .map_err(|_| Error::Deserialize)
                    .and_then(|decoded| {
                        self.incoming_packet(packet.addr(), decoded)
                    })
                    .or_else(|error| {
                        eprintln!("! error/packet decode from {}: {}", packet.addr(), error);
                        Err(error)
                    })
            },
            SocketEvent::Connect(addr) => {
                eprintln!("* connected: {}", addr);
                Ok(())
            },
            SocketEvent::Timeout(addr) | SocketEvent::Disconnect(addr) => {
                eprintln!("* disconnected: {}", addr);
                self.disconnect(addr);
                Ok(())
            },
        }
    }

}

fn main() -> Result<(), Box<dyn error::Error>> {

    eprintln!("NetCanv Matchmaker: starting on port {}", PORT);

    let localhost = SocketAddr::from(([127, 0, 0, 1], PORT));
    let mut socket = Socket::bind(localhost)?;
    let (tx, rx) = (socket.get_packet_sender(), socket.get_event_receiver());
    let _thread = std::thread::spawn(move || socket.start_polling());

    let mut state = Matchmaker::new();

    eprintln!("Listening for incoming connections");

    loop {
        // receive
        rx.recv()
            .map_err(|_| Error::Recv)
            .and_then(|event| {
                // process
                let has_response = matches!(&event, SocketEvent::Packet(_));
                let result = state.incoming_event(event);
                // transmit
                if has_response {
                    state.send_queue.send(&tx)?;
                }
                result
            })
            .or_else(|error| -> Result<_, ()> {
                eprintln!("! error/recv: {:?}", error);
                Ok(())
            })
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forge_packet_event(addr: SocketAddr, from: Packet) -> SocketEvent {
        let payload = bincode::serialize(&from).unwrap();
        SocketEvent::Packet(laminar::Packet::reliable_ordered(addr, payload, None))
    }

    // this also tests whether the responses can be serialized correctly
    fn get_responses(state: &mut Matchmaker, addr: SocketAddr) -> Vec<Packet> {
        let encoded = state.send_queue.serialize(addr).unwrap();
        let decoded: Vec<Packet> = bincode::deserialize(&encoded).unwrap();
        decoded
    }

    const HOST_ADDRESS: ([u8; 4], u16) = ([192, 168, 1, 24], 62137);
    const CLIENT_ADDRESS: ([u8; 4], u16) = ([192, 168, 1, 25], 62137);

    #[test]
    fn create_room() {
        let host = SocketAddr::from(HOST_ADDRESS);
        let mut state = Matchmaker::new();
        let packet = forge_packet_event(host, Packet::Host);
        state.incoming_event(packet).unwrap();
        let responses = get_responses(&mut state, host);
        assert!(matches!(responses[0], Packet::RoomId(0..=MAX_ROOM_ID)));
    }

    #[test]
    fn join_room() {
        let host = SocketAddr::from(HOST_ADDRESS);
        let client = SocketAddr::from(CLIENT_ADDRESS);

        let mut state = Matchmaker::new();

        let host_packet = forge_packet_event(host, Packet::Host);
        state.incoming_event(host_packet).unwrap();
        let room_id_packet = get_responses(&mut state, host)[0].clone();
        let room_id =
            if let Packet::RoomId(id) = room_id_packet { id }
            else { unreachable!() };
        state.send_queue.clear();

        let get_host_packet = forge_packet_event(client, Packet::GetHost(room_id));
        state.incoming_event(get_host_packet).unwrap();
        let host_addr_packet = get_responses(&mut state, client)[0].clone();
        let client_addr_packet = get_responses(&mut state, host)[0].clone();
        state.send_queue.clear();

        if let
           (Packet::HostAddress(host_addr_string), Packet::ClientAddress(client_addr_string))
           = (host_addr_packet, client_addr_packet)
        {
            let host_address: SocketAddr = host_addr_string.parse().unwrap();
            let client_address: SocketAddr = client_addr_string.parse().unwrap();
            assert_eq!(host, host_address);
            assert_eq!(client, client_address);
        } else {
            unreachable!();
        }
    }
}
