// use std::net::{SocketAddr, ToSocketAddrs, TcpStream};
// use std::thread;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use netcanv_protocol::client as cl;
use netcanv_protocol::matchmaker as mm;
use skulpin::skia_safe::{Color, Color4f, Point};

use crate::net::socket::Remote;
use crate::paint_canvas::{Brush, StrokePoint};
use crate::util;

#[derive(Debug)]
pub enum Message {
    //
    // general
    //

    // return to the lobby with an error message
    Error(String),

    //
    // connection
    //

    // created a room or connected to the host
    Connected,

    //
    // painting
    //

    // someone has joined, maybe send them all chunk positions
    Joined(String, Option<SocketAddr>),

    // someone has left
    Left(String),

    // stroke packet received
    Stroke(Vec<StrokePoint>),

    // ChunkPositions packet received
    ChunkPositions(Vec<(i32, i32)>),

    // GetChunks packet received
    GetChunks(SocketAddr, Vec<(i32, i32)>),

    // a Chunks-compatible packet received
    Chunks(Vec<((i32, i32), Vec<u8>)>),
}

pub struct Mate {
    pub cursor: Point,
    pub cursor_prev: Point,
    pub last_cursor: Instant,
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
    host: Option<SocketAddr>,
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
    pub fn host(nickname: &str, matchmaker_addr: &str) -> anyhow::Result<Self> {
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
            host: None,
        })
    }

    pub fn join(nickname: &str, matchmaker_addr: &str, room_id: u32) -> anyhow::Result<Self> {
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
            host: None,
        })
    }

    // is_relayed is an output variable to appease the borrow checker. can't borrow &mut self because of
    // the literal first borrow in next_packet
    fn connect_to_host(mm: &Remote<mm::Packet>, host_addr: SocketAddr, is_relayed: &mut bool) -> anyhow::Result<()> {
        // for now we'll always relay packets because i don't think it's possible to do hole punching with
        // rust's stdlib TcpStream
        mm.send(mm::Packet::RequestRelay(Some(host_addr)))?;
        *is_relayed = true;
        Ok(())
    }

    fn send(&self, to: Option<SocketAddr>, packet: cl::Packet) -> anyhow::Result<()> {
        // TODO: no matchmaker relay
        self.matchmaker
            .as_ref()
            .unwrap()
            .send(mm::Packet::Relay(to, bincode::serialize(&packet)?))?;
        Ok(())
    }

    fn add_mate(&mut self, addr: SocketAddr, nickname: String) {
        self.mates.insert(addr, Mate {
            cursor: Point::new(0.0, 0.0),
            cursor_prev: Point::new(0.0, 0.0),
            last_cursor: Instant::now(),
            nickname,
            brush_size: 4.0,
        });
    }

    fn decode_payload(&mut self, sender_addr: SocketAddr, payload: &[u8]) -> Option<Message> {
        let packet = match bincode::deserialize::<cl::Packet>(payload) {
            Err(_) if self.is_host() => return None,
            Err(error) =>
                return Some(Message::Error(format!(
                    "Unknown packet ({}). Check if your client is up to date",
                    error
                ))),
            Ok(packet) => packet,
        };

        match packet {
            //
            // 0.1.0
            cl::Packet::Hello(nickname) => {
                eprintln!("{} ({}) joined", nickname, sender_addr);
                try_or_message!(self.send(Some(sender_addr), cl::Packet::HiThere(self.nickname.clone())));
                try_or_message!(self.send(Some(sender_addr), cl::Packet::Version(cl::PROTOCOL_VERSION)));
                self.add_mate(sender_addr, nickname.clone());
                return Some(Message::Joined(nickname, self.is_host.then(|| sender_addr)))
            },
            cl::Packet::HiThere(nickname) => {
                eprintln!("{} ({}) is in the room", nickname, sender_addr);
                self.add_mate(sender_addr, nickname);
            },
            cl::Packet::Cursor(x, y, brush_size) =>
                if let Some(mate) = self.mates.get_mut(&sender_addr) {
                    mate.cursor_prev = mate.cursor;
                    mate.cursor = Point::new(cl::from_fixed29p3(x), cl::from_fixed29p3(y));
                    mate.last_cursor = Instant::now();
                    mate.brush_size = cl::from_fixed15p1(brush_size);
                } else {
                    eprintln!("{} sus", sender_addr);
                },
            cl::Packet::Stroke(points) => {
                let points: Vec<StrokePoint> = points
                    .iter()
                    .map(|p| StrokePoint {
                        point: Point::new(cl::from_fixed29p3(p.x), cl::from_fixed29p3(p.y)),
                        brush: if p.color == 0 {
                            Brush::Erase {
                                stroke_width: cl::from_fixed15p1(p.brush_size),
                            }
                        } else {
                            Brush::Draw {
                                color: Color4f::from(Color::new(p.color)),
                                stroke_width: cl::from_fixed15p1(p.brush_size),
                            }
                        },
                    })
                    .collect();
                if points.len() > 0 {
                    if let Some(mate) = self.mates.get_mut(&sender_addr) {
                        mate.cursor_prev = points[0].point;
                        mate.cursor = points.last().unwrap().point;
                        mate.last_cursor = Instant::now();
                    }
                }
                return Some(Message::Stroke(points))
            },
            cl::Packet::Reserved => (),
            // 0.2.0
            cl::Packet::Version(version) if !cl::compatible_with(version) =>
                return Some(Message::Error("Client is too old.".into())),
            cl::Packet::Version(_) => (),
            cl::Packet::ChunkPositions(positions) => return Some(Message::ChunkPositions(positions)),
            cl::Packet::GetChunks(positions) => return Some(Message::GetChunks(sender_addr, positions)),
            cl::Packet::Chunks(chunks) => return Some(Message::Chunks(chunks)),
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
                        self.host = Some(*addr);
                        then = Then::SayHello;
                        message = Some(
                            Self::connect_to_host(mm, *addr, &mut self.is_relayed)
                                .err()
                                .map_or(Message::Connected, |e| Message::Error(format!("{}", e))),
                        );
                    },
                    mm::Packet::ClientAddress(addr) => (),
                    mm::Packet::Relayed(_, payload) if payload.len() == 0 => then = Then::SayHello,
                    mm::Packet::Relayed(from, payload) => then = Then::ReadRelayed(*from, payload.to_vec()),
                    mm::Packet::Disconnected(addr) =>
                        if let Some(mate) = self.mates.remove(&addr) {
                            return Some(Message::Left(mate.nickname))
                        },
                    mm::Packet::Error(message) => return Some(Message::Error(message.into())),
                    _ => return None,
                }
            }
        }

        match then {
            Then::Continue => (),
            Then::ReadRelayed(sender, payload) => return self.decode_payload(sender, &payload),
            Then::SayHello => {
                try_or_message!(self.send(None, cl::Packet::Hello(self.nickname.clone())))
            },
        }

        message
    }

    pub fn tick<'a>(&'a mut self) -> anyhow::Result<Messages<'a>> {
        if let Some(mm) = &self.matchmaker {
            let _ = mm.tick()?;
        }
        Ok(Messages { peer: self })
    }

    pub fn send_cursor(&self, cursor: Point, brush_size: f32) -> anyhow::Result<()> {
        self.send(
            None,
            cl::Packet::Cursor(
                cl::to_fixed29p3(cursor.x),
                cl::to_fixed29p3(cursor.y),
                cl::to_fixed15p1(brush_size),
            ),
        )
    }

    pub fn send_stroke(&self, iterator: impl Iterator<Item = StrokePoint>) -> anyhow::Result<()> {
        self.send(
            None,
            cl::Packet::Stroke(
                iterator
                    .map(|p| cl::StrokePoint {
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
                    })
                    .collect(),
            ),
        )
    }

    pub fn send_chunk_positions(&self, to: SocketAddr, positions: Vec<(i32, i32)>) -> anyhow::Result<()> {
        self.send(Some(to), cl::Packet::ChunkPositions(positions))
    }

    pub fn download_chunks(&self, positions: Vec<(i32, i32)>) -> anyhow::Result<()> {
        assert!(self.host.is_some(), "only non-hosts can download chunks");
        eprintln!("downloading {} chunks from the host", positions.len());
        self.send(self.host, cl::Packet::GetChunks(positions))
    }

    pub fn send_chunks(&self, to: SocketAddr, chunks: Vec<((i32, i32), Vec<u8>)>) -> anyhow::Result<()> {
        self.send(Some(to), cl::Packet::Chunks(chunks))
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

impl Mate {
    pub fn lerp_cursor(&self) -> Point {
        use crate::app::paint::State;
        let elapsed_ms = self.last_cursor.elapsed().as_millis() as f32;
        let t = (elapsed_ms / State::TIME_PER_UPDATE.as_millis() as f32).min(1.0);
        util::lerp_point(self.cursor_prev, self.cursor, t)
    }
}
