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

    fn enqueue_response(&mut self, dest_addr: SocketAddr, packet: Packet) {
        self.send_queue.enqueue(dest_addr, packet);
    }

    fn enqueue_error(&mut self, dest_addr: SocketAddr, message: &str) {
        self.enqueue_response(dest_addr, error_packet(message));
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

    fn host(&mut self, addr: SocketAddr) {
        match self.find_free_room_id() {
            Some(room_id) => {
                let host = Host { addr, room_id };
                self.hosts.insert(addr, host.clone());
                self.rooms.insert(room_id, host);
                self.enqueue_response(addr, Packet::RoomId(room_id));
            },
            None => self.enqueue_error(addr, "Could not find any more free rooms. Try again"),
        }
    }

    fn incoming_packet(&mut self, addr: SocketAddr, packet: Packet) -> Result<(), Error> {
        match packet {
            Packet::Host => self.host(addr),
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

    fn forge_packet_event(addr: &str, from: Packet) -> SocketEvent {
        let payload = bincode::serialize(&from).unwrap();
        let addr: SocketAddr = addr.parse().unwrap();
        SocketEvent::Packet(laminar::Packet::reliable_ordered(addr, payload, None))
    }

    // this also tests whether the responses can be serialized correctly
    fn get_responses(state: &mut Matchmaker) -> Vec<Packet> {
        let responses = state.response_queue.clone();
        state.response_queue.clear();
        let encoded = bincode::serialize(&responses).unwrap();
        let decoded: Vec<Packet> = bincode::deserialize(&encoded).unwrap();
        assert_eq!(responses, decoded);
        decoded
    }

    const TEST_ADDRESS: &str = "127.0.0.1:62137";

    #[test]
    fn create_room() {
        let mut state = Matchmaker::new();
        let packet = forge_packet_event(TEST_ADDRESS, Packet::Host);
        state.incoming_event(packet).0.unwrap();
        let responses = get_responses(&mut state);
        assert!(matches!(responses[0], Packet::RoomId(0..=MAX_ROOM_ID)));
    }
}
