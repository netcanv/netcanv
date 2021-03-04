// the netcanv matchmaker server.
// keeps track of open rooms and exchanges addresses between hosts and their clients

use std::collections::{HashMap};
use std::error;
use std::net::SocketAddr;

use laminar::{Socket, SocketEvent};

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
    response_queue: Vec<Packet>,
}

#[derive(Debug)]
enum Error {
    Recv,
    InvalidPacket,
    Deserialize,
}

impl Matchmaker {

    fn new() -> Self {
        Self {
            hosts: HashMap::new(),
            rooms: HashMap::new(),
            response_queue: Vec::new(),
        }
    }

    fn enqueue_response(&mut self, packet: Packet) {
        self.response_queue.push(packet);
    }

    fn enqueue_error(&mut self, message: &str) {
        self.response_queue.push(error_packet(message));
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
                self.enqueue_response(Packet::RoomId(room_id));
            },
            None => self.enqueue_error("Could not find any more free rooms. Try again"),
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

    fn incoming_event(&mut self, event: SocketEvent) -> (Result<(), Error>, SocketAddr) {
        match event {
            SocketEvent::Packet(packet) => {
                let payload = packet.payload();
                match bincode::deserialize(payload) {
                    Ok(decoded) => (self.incoming_packet(packet.addr(), decoded), packet.addr()),
                    Err(error) => {
                        eprintln!("! error/packet decode from {}: {}", packet.addr(), error);
                        (Err(Error::Deserialize), packet.addr())
                    },
                }
            },
            SocketEvent::Connect(addr) => {
                eprintln!("* connected: {}", addr);
                (Ok(()), addr)
            },
            SocketEvent::Timeout(addr) | SocketEvent::Disconnect(addr) => {
                eprintln!("* disconnected: {}", addr);
                self.disconnect(addr);
                (Ok(()), addr)
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

    let error_response_payload = bincode::serialize(&[0u32])?;

    eprintln!("Listening for incoming connections");

    loop {
        // receive
        let result = rx.recv()
            .map_err(|_| Error::Recv)
            .and_then(|event| {
                // process
                let has_response = matches!(&event, SocketEvent::Packet(_));
                let (result, addr) = state.incoming_event(event);
                // transmit
                if has_response {
                    assert!(state.response_queue.len() > 0);
                    let packet: laminar::Packet = match bincode::serialize(&state.response_queue) {
                        Ok(response_payload) =>
                            laminar::Packet::reliable_ordered(addr, response_payload, None),
                        Err(error) => {
                            eprintln!("! error/send: {}", error);
                            laminar::Packet::reliable_ordered(addr, error_response_payload.clone(), None)
                        },
                    };
                    tx.send(packet).expect("failed to send packet to the receiving channel. not my fault.");
                }
                result
            });
        if let Err(error) = result {
            eprintln!("! error/recv: {:?}", error);
        }

        state.response_queue.clear();
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
