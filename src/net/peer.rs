// use std::net::{SocketAddr, ToSocketAddrs, TcpStream};
// use std::thread;

use std::net::ToSocketAddrs;

use crossbeam_channel::{Receiver, Sender, SendError};
use thiserror::Error;

use crate::net::socket::{Remote, Error as NetError};
use netcanv_protocol::matchmaker as mm;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Net(#[from] NetError),
}

pub enum Message {
    //
    // general
    //

    // i wonder what this could mean
    Error(String),

    //
    // connection
    //

    // created a room or connected to the host
    Connected,
}

pub struct Peer {
    matchmaker: Option<Remote<mm::Packet>>,
    is_self: bool,
    is_host: bool,
    room_id: Option<u32>,
}

pub struct Messages<'a> {
    peer: &'a mut Peer,
}

impl Peer {

    pub fn host(matchmaker_addr: &str) -> Result<Self, Error> {
        let mm = Remote::new(matchmaker_addr)?;
        mm.send(mm::Packet::Host)?;

        Ok(Self {
            matchmaker: Some(mm),
            is_self: true,
            is_host: true,
            room_id: None,
        })
    }

    pub fn join(matchmaker_addr: &str, room_id: u32) -> Result<Self, Error> {
        let mm = Remote::new(matchmaker_addr)?;
        mm.send(mm::Packet::GetHost(room_id))?;

        Ok(Self {
            matchmaker: Some(mm),
            is_self: true,
            is_host: false,
            room_id: None,
        })
    }

    fn connect_to_host(&mut self, host_addr: impl ToSocketAddrs) {
        // todo
    }

    fn next_packet(&mut self) -> Option<Message> {
        if let Some(mm) = &self.matchmaker {
            // give me back my if-let-chaining
            if let Some(packet) = &mm.try_recv() {
                return match packet {
                    mm::Packet::RoomId(id) => {
                        self.room_id = Some(*id);
                        Some(Message::Connected)
                    },
                    mm::Packet::Error(message) => Some(Message::Error(message.into())),
                    mm::Packet::HostAddress(addr) => {
                        self.connect_to_host(addr);
                        None
                    },
                    _ => None,
                }
            }
        }

        None
    }

    pub fn tick<'a>(&'a mut self) -> Result<Messages<'a>, Error> {
        if let Some(mm) = &self.matchmaker {
            let _ = mm.tick()?;
        }
        Ok(Messages {
            peer: self,
        })
    }

    pub fn is_host(&self) -> bool {
        self.is_host
    }

    // this will return None if we're not connected yet
    pub fn room_id(&self) -> Option<u32> {
        self.room_id
    }

}

impl Iterator for Messages<'_> {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        self.peer.next_packet()
    }
}

