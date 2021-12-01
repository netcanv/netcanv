//! An abstraction for sockets, communicating over the global bus.

use std::fmt::Debug;
use std::io::{BufWriter, Write};
use std::marker::PhantomData;
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use nysa::global as bus;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::token::Token;

/// A token for connecting a socket asynchronously.
///
/// Once a socket connects successfully, [`Connected`] is pushed onto the bus, containing this
/// token and the socket handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionToken(usize);

/// A successful connection message.
pub struct Connected<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   pub token: ConnectionToken,
   pub socket: Socket<T>,
}

/// A message pushed onto the bus when there's a new packet incoming from a socket.
pub struct IncomingPacket<T>
where
   T: DeserializeOwned,
{
   pub token: ConnectionToken,
   pub data: T,
}

/// A message to the network subsystem that a packet should be sent with the given data.
enum SendPacket<T>
where
   T: DeserializeOwned + Serialize,
{
   Packet(IncomingPacket<T>),
   Quit(ConnectionToken),
}

/// A message asking the rx thread with associated token to shut down.
struct QuitReceive(ConnectionToken);

/// A trait describing a valid, (de)serializable, owned packet.
pub trait Packet: 'static + Send + DeserializeOwned + Serialize {}

/// A unique handle to a socket.
//
// These handles cannot be cloned or copied, as each handle owns a single socket thread.
// Once a handle is dropped, its associated thread is also asked to quit, and joined to the calling
// thread.
pub struct Socket<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   token: ConnectionToken,
   system: Arc<SocketSystem<T>>,
   thread_slot: usize,
}

impl<T> Socket<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   /// Returns the socket's connection token.
   pub fn token(&self) -> ConnectionToken {
      self.token
   }

   /// Issues a request that a packet with the provided data should be serialized and sent over the
   /// socket.
   pub fn send(&self, data: T) {
      bus::push(SendPacket::Packet(IncomingPacket {
         token: self.token,
         data,
      }))
   }
}

impl<T> Drop for Socket<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   fn drop(&mut self) {
      bus::push(SendPacket::Quit::<T>(self.token));

      let mut system_inner = self.system.inner.lock().unwrap();
      let (receiving, sending) = system_inner.socket_threads[self.thread_slot].take().unwrap();
      receiving.join().expect("receiving thread panicked");
      sending.join().expect("sending thread panicked");
   }
}

/// A socket handling subsystem for the given packet type `T`.
pub struct SocketSystem<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   inner: Mutex<SocketSystemInner<T>>,
}

static CONNECTION_TOKEN: Token = Token::new();

impl<T> SocketSystem<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   pub fn new() -> Arc<Self> {
      Arc::new(Self {
         inner: Mutex::new(SocketSystemInner::new()),
      })
   }

   fn resolve_address_with_default_port(
      address: &str,
      default_port: u16,
   ) -> anyhow::Result<Vec<SocketAddr>> {
      Ok(if let Ok(addresses) = address.to_socket_addrs() {
         addresses.collect()
      } else {
         (address, default_port).to_socket_addrs()?.collect()
      })
   }

   pub fn connect(
      self: &Arc<Self>,
      address: String,
      default_port: u16,
   ) -> anyhow::Result<ConnectionToken> {
      let token = ConnectionToken(CONNECTION_TOKEN.next());

      let this = Arc::clone(self);
      thread::Builder::new().name("network connection thread".into()).spawn(move || {
         let thread_slot = {
            let mut inner = this.inner.lock().unwrap();
            let addresses = catch!(Self::resolve_address_with_default_port(
               &address,
               default_port
            ));
            catch!(inner.connect(token, &addresses[..]))
         };
         let socket = Socket {
            token,
            system: this,
            thread_slot,
         };
         bus::push(Connected { token, socket });
      })?;

      Ok(token)
   }
}

/// A socket slot containing join handles for the receiving and sending thread, respectively.
type Slot = Option<(JoinHandle<()>, JoinHandle<()>)>;

/// The inner, non thread-safe data of `SocketSystem`.
struct SocketSystemInner<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   socket_threads: Vec<Slot>,
   _phantom_data: PhantomData<T>,
}

impl<T> SocketSystemInner<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   // This "inner" version of SocketSystem contains methods that operate on the inner vec of socket
   // threads. These are "raw" versions of the public, safe API.

   fn new() -> Self {
      Self {
         socket_threads: Vec::new(),
         _phantom_data: PhantomData,
      }
   }

   fn find_free_slot(&self) -> Option<usize> {
      self.socket_threads.iter().position(|slot| slot.is_none())
   }

   fn connect(
      &mut self,
      token: ConnectionToken,
      address: impl ToSocketAddrs,
   ) -> anyhow::Result<usize> {
      let stream = TcpStream::connect(address)?;
      stream.set_nodelay(true)?;

      const KILOBYTE: usize = 1024;

      // Reading and writing is buffered so as not to slow down performance when big packets are sent.

      let reader = stream.try_clone()?; // BufReader::with_capacity(64 * KILOBYTE, stream.try_clone()?);
      let receiving_thread =
         thread::Builder::new().name("network receiving thread".into()).spawn(move || loop {
            // Quit when the owning socket's dropped.
            for message in &bus::retrieve_all::<QuitReceive>() {
               if message.0 == token {
                  message.consume();
                  return;
               }
            }
            // Read packets from the stream. `deserialize_from` will block until a packet is read successfully.
            let data: T = catch!(bincode::deserialize_from(&reader));
            bus::push(IncomingPacket { token, data });
         })?;

      let mut writer = BufWriter::with_capacity(64 * KILOBYTE, stream.try_clone()?);
      let sending_thread =
         thread::Builder::new().name("network sending thread".into()).spawn(move || loop {
            let message = bus::wait_for::<SendPacket<T>>();
            match &*message {
               // Serialize and send the packet.
               SendPacket::Packet(packet) if packet.token == token => {
                  let data = catch!(bincode::serialize(&packet.data));
                  let length = [
                     (data.len() >> 24) as u8,
                     (data.len() >> 16) as u8,
                     (data.len() >> 8) as u8,
                     (data.len()) as u8,
                  ];
                  catch!(writer.write_all(&length));
                  catch!(writer.write_all(&data));
                  catch!(writer.flush());
                  message.consume();
               }
               // Quit when the owning socket is dropped.
               SendPacket::Quit(quit_token) if *quit_token == token => {
                  // The read stream is most certainly still blocking, trying to read a packet.
                  // Thus, we shutdown the stream completely, which should make it stop reading.
                  catch!(stream.shutdown(Shutdown::Both));
                  message.consume();
                  return;
               }
               _ => (),
            }
         })?;

      Ok(match self.find_free_slot() {
         Some(slot) => {
            self.socket_threads[slot] = Some((receiving_thread, sending_thread));
            slot
         }
         None => {
            let slot = self.socket_threads.len();
            self.socket_threads.push(Some((receiving_thread, sending_thread)));
            slot
         }
      })
   }
}
