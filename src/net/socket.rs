//! An abstraction for sockets, communicating over the global bus.

use std::cell::RefCell;
use std::fmt::Display;
use std::io::{BufReader, BufWriter, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use nysa::global as bus;

/// A token for connecting a socket asynchronously.
///
/// Once a socket connects successfully, [`Connected`] is pushed onto the bus, containing this
/// token and the socket handle.
pub struct ConnectionToken(usize);

/// A successful connection message.
pub struct Connected(ConnectionToken, Socket);

/// A unique handle to a socket.
// These handles cannot be cloned or copied, as each handle owns a single socket thread.
// Once a handle is dropped, its associated thread is also
pub struct Socket {
    system: Arc<SocketSystem>,
    thread_slot: usize,
}

/// The socket handling subsystem.
pub struct SocketSystem {
    inner: Mutex<SocketSystemInner>,
}

impl SocketSystem {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SocketSystemInner::new()),
        }
    }
}

/// A socket slot containing join handles for the receiving and sending thread, respectively.
type Slot = Option<(JoinHandle<()>, JoinHandle<()>)>;

/// The inner, non thread-safe data of `SocketSystem`.
struct SocketSystemInner {
    socket_threads: Vec<Slot>,
}

impl SocketSystemInner {
    // This "inner" version of SocketSystem contains methods that operate on the inner vec of socket
    // threads. These are "raw" versions of the public, safe API.

    fn new() -> Self {
        Self {
            socket_threads: Vec::new(),
        }
    }

    fn find_free_slot(&self) -> Option<usize> {
        self.socket_threads.iter().position(|slot| slot.is_none())
    }

    fn connect(&mut self, address: impl ToSocketAddrs) -> anyhow::Result<usize> {
        let stream = Arc::new(TcpStream::connect(address)?);

        let receiving_thread = {
            let stream = Arc::clone(&stream);
            std::thread::Builder::new()
                .name("network receiving thread".into())
                .spawn(move || {})?
        };

        let sending_thread = std::thread::Builder::new()
            .name("network sending thread".into())
            .spawn(move || {})?;

        Ok(match self.find_free_slot() {
            Some(slot) => {
                self.socket_threads[slot] = Some((receiving_thread, sending_thread));
                slot
            },
            None => {
                let slot = self.socket_threads.len();
                self.socket_threads.push(Some((receiving_thread, sending_thread)));
                slot
            },
        })
    }
}
