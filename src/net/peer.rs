// use std::net::{SocketAddr, ToSocketAddrs, TcpStream};
// use std::thread;

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
    // matchmaker
    //

    // host→mm - response from the matchmaker with a room ID
    RoomId(u32),
}

pub struct Peer {
    matchmaker: Option<Remote<mm::Packet>>,
    is_self: bool,
}

pub struct Messages<'a> {
    peer: &'a Peer,
}

impl Peer {

    pub fn host(matchmaker_addr: &str) -> Result<Self, Error> {
        let mm = Remote::new(matchmaker_addr)?;
        mm.send(mm::Packet::Host)?;

        Ok(Self {
            matchmaker: Some(mm),
            is_self: true,
        })
    }

    pub fn join(matchmaker_addr: &str, room_id: u32) -> Result<Self, Error> {
        let mm = Remote::new(matchmaker_addr)?;
        mm.send(mm::Packet::GetHost(room_id));

        Ok(Self {
            matchmaker: Some(mm),
            is_self: true,
        })
    }

    fn next_packet(&self) -> Option<Message> {
        if let Some(mm) = &self.matchmaker {
            // give me back my if-let-chaining
            if let Some(packet) = &mm.try_recv() {
                return match packet {
                    mm::Packet::RoomId(id) => Some(Message::RoomId(*id)),
                    mm::Packet::Error(message) => Some(Message::Error(message.into())),
                    mm::Packet::HostAddress(addy) => {
                        println!("host address: {}", addy);
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

}

impl Iterator for Messages<'_> {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        self.peer.next_packet()
    }
}

