// use std::net::{SocketAddr, ToSocketAddrs, TcpStream};
// use std::thread;

use std::collections::HashMap;
use std::net::{SocketAddr};

use skulpin::skia_safe::{Color, Color4f, Point};
use thiserror::Error;

use crate::net::socket::{Remote, Error as NetError};
use crate::paint_canvas::{Brush, StrokePoint};
use netcanv_protocol::client as cl;
use netcanv_protocol::matchmaker as mm;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Net(#[from] NetError),
    #[error("Data error: {0}")]
    Data(#[from] bincode::Error),
}

#[derive(Debug)]
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

    //
    // painting
    //

    // someone has joined
    Joined(String),

    // someone has left
    Left(String),

    // a new mate has arrived in the room and needs canvas data
    NewMate(SocketAddr),

    // stroke packet received
    Stroke(Vec<StrokePoint>),

    // canvas data packet received
    CanvasData((i32, i32), Vec<u8>)
}

pub struct Mate {
    pub cursor: Point,
    pub nickname: String,
    pub brush_size: f32,
}

pub struct Peer {
    matchmaker: Option<Remote<mm::Packet>>,
    is_self: bool,
    is_host: bool,
    is_relayed: bool,
    nickname: String,
    room_id: Option<u32>,
    mates: HashMap<SocketAddr, Mate>,
}

pub struct Messages<'a> {
    peer: &'a mut Peer,
}

macro_rules! try_or_message {
    ($exp:expr, $fmt:literal) => {
        match $exp {
            Ok(x) => x,
            Err(e) => return Some(Message::Error(format!($fmt, e))),
        }
    };
    ($exp:expr) => {
        try_or_message!($exp, "{}")
    };
}

impl Peer {

    pub fn host(nickname: &str, matchmaker_addr: &str) -> Result<Self, Error> {
        let mm = Remote::new(matchmaker_addr)?;
        mm.send(mm::Packet::Host)?;

        Ok(Self {
            matchmaker: Some(mm),
            is_self: true,
            is_host: true,
            is_relayed: false,
            nickname: nickname.into(),
            room_id: None,
            mates: HashMap::new(),
        })
    }

    pub fn join(nickname: &str, matchmaker_addr: &str, room_id: u32) -> Result<Self, Error> {
        let mm = Remote::new(matchmaker_addr)?;
        mm.send(mm::Packet::GetHost(room_id))?;

        Ok(Self {
            matchmaker: Some(mm),
            is_self: true,
            is_host: false,
            is_relayed: false,
            nickname: nickname.into(),
            room_id: None,
            mates: HashMap::new(),
        })
    }

    // is_relayed is an output variable to appease the borrow checker. can't borrow &mut self because of the literal
    // first borrow in next_packet
    fn connect_to_host(mm: &Remote<mm::Packet>, host_addr: SocketAddr, is_relayed: &mut bool) -> Result<(), Error> {
        // for now we'll always relay packets because i don't think it's possible to do hole punching with
        // rust's stdlib TcpStream
        mm.send(mm::Packet::RequestRelay(Some(host_addr)))?;
        *is_relayed = true;
        Ok(())
    }

    fn send(&self, to: Option<SocketAddr>, packet: cl::Packet) -> Result<(), Error> {
        // TODO: no matchmaker relay
        self.matchmaker
            .as_ref()
            .unwrap()
            .send(mm::Packet::Relay(to, bincode::serialize(&packet)?))?;
        Ok(())
    }

    fn add_mate(&mut self, addr: SocketAddr, nickname: String) {
        self.mates.insert(addr, Mate {
            nickname,
            cursor: Point::new(0.0, 0.0),
            brush_size: 4.0,
        });
    }

    fn decode_payload(&mut self, sender_addr: SocketAddr, payload: &[u8]) -> Option<Message> {
        let packet = try_or_message!(bincode::deserialize::<cl::Packet>(payload), "Invalid packet received: {}");

        match packet {
            cl::Packet::Hello(nickname) => {
                eprintln!("{} ({}) joined", nickname, sender_addr);
                try_or_message!(self.send(Some(sender_addr), cl::Packet::HiThere(self.nickname.clone())));
                self.add_mate(sender_addr, nickname.clone());
                return Some(Message::Joined(nickname))
            },
            cl::Packet::HiThere(nickname) => {
                eprintln!("{} ({}) is in the room", nickname, sender_addr);
                self.add_mate(sender_addr, nickname);
            },
            cl::Packet::Cursor(x, y, brush_size) => {
                if let Some(mate) = self.mates.get_mut(&sender_addr) {
                    mate.cursor = Point::new(cl::from_fixed29p3(x), cl::from_fixed29p3(y));
                    mate.brush_size = cl::from_fixed15p1(brush_size);
                } else {
                    eprintln!("{} sus", sender_addr);
                }
            },
            cl::Packet::Stroke(points) => {
                return Some(Message::Stroke(points.into_iter().map(|p| {
                    StrokePoint {
                        point: Point::new(cl::from_fixed29p3(p.x), cl::from_fixed29p3(p.y)),
                        brush:
                            if p.color == 0 {
                                Brush::Erase { stroke_width: cl::from_fixed15p1(p.brush_size) }
                            } else {
                                Brush::Draw {
                                    color: Color4f::from(Color::new(p.color)),
                                    stroke_width: cl::from_fixed15p1(p.brush_size)
                                }
                            }
                    }
                }).collect()));
            },
            cl::Packet::CanvasData(chunk, png_image) => {
                return Some(Message::CanvasData(chunk, png_image));
            },
        }

        None
    }

    fn next_packet(&mut self) -> Option<Message> {
        enum Then {
            Continue,
            ReadRelayed(SocketAddr, Vec<u8>),
            SayHello,
        }
        let mut then = Then::Continue;
        let mut message: Option<Message> = None;

        if let Some(mm) = &self.matchmaker {
            // give me back my if-let-chaining
            if let Some(packet) = &mm.try_recv() {
                match packet {
                    mm::Packet::RoomId(id) => {
                        self.room_id = Some(*id);
                        try_or_message!(mm.send(mm::Packet::RequestRelay(None)));
                        then = Then::SayHello;
                        message = Some(Message::Connected);
                    },
                    mm::Packet::HostAddress(addr) => {
                        message = Some(
                            Self::connect_to_host(mm, *addr, &mut self.is_relayed)
                                .err()
                                .map_or(
                                    Message::Connected,
                                    |e| Message::Error(format!("{}", e)),
                                )
                        );
                        if let Some(Message::Connected) = message {
                            then = Then::SayHello;
                        }
                    },
                    mm::Packet::ClientAddress(addr) => return Some(Message::NewMate(*addr)),
                    mm::Packet::Relayed(from, payload) => then = Then::ReadRelayed(*from, payload.to_vec()),
                    mm::Packet::Disconnected(addr) => {
                        if let Some(mate) = self.mates.remove(&addr) {
                            return Some(Message::Left(mate.nickname))
                        }
                    },
                    mm::Packet::Error(message) => return Some(Message::Error(message.into())),
                    _ => return None,
                }
            }
        }

        match then {
            Then::Continue => (),
            Then::ReadRelayed(sender, payload) => return self.decode_payload(sender, &payload),
            Then::SayHello => try_or_message!(self.send(None, cl::Packet::Hello(self.nickname.clone()))),
        }

        message
    }

    pub fn tick<'a>(&'a mut self) -> Result<Messages<'a>, Error> {
        if let Some(mm) = &self.matchmaker {
            let _ = mm.tick()?;
        }
        Ok(Messages {
            peer: self,
        })
    }

    pub fn send_cursor(&self, cursor: Point, brush_size: f32) -> Result<(), Error> {
        self.send(None, cl::Packet::Cursor(
            cl::to_fixed29p3(cursor.x),
            cl::to_fixed29p3(cursor.y),
            cl::to_fixed15p1(brush_size)
        ))
    }

    pub fn send_stroke(&self, iterator: impl Iterator<Item = StrokePoint>) -> Result<(), Error> {
        self.send(None, cl::Packet::Stroke(iterator.map(|p| {
            cl::StrokePoint {
                x: cl::to_fixed29p3(p.point.x),
                y: cl::to_fixed29p3(p.point.y),
                color: match p.brush {
                    Brush::Draw { ref color, .. } => {
                        let color = color.to_color();
                        ((color.a() as u32) << 24) |
                        ((color.r() as u32) << 16) |
                        ((color.g() as u32) << 8) |
                        color.b() as u32
                    },
                    Brush::Erase { .. } => 0,
                },
                brush_size: cl::to_fixed15p1(match p.brush {
                    Brush::Draw { stroke_width, .. } | Brush::Erase { stroke_width } => stroke_width,
                }),
            }
        }).collect()))
    }

    pub fn send_canvas_data(&self, to: SocketAddr, chunk: (i32, i32), png_data: Vec<u8>) -> Result<(), Error> {
        self.send(Some(to), cl::Packet::CanvasData(chunk, png_data))
    }

    pub fn is_host(&self) -> bool {
        self.is_host
    }

    // this will return None if we're not connected yet
    pub fn room_id(&self) -> Option<u32> {
        self.room_id
    }

    pub fn mates(&self) -> &HashMap<SocketAddr, Mate> {
        &self.mates
    }

}

impl Iterator for Messages<'_> {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        self.peer.next_packet()
    }
}

