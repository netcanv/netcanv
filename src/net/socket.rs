//! An abstraction for sockets, communicating over the global bus.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use instant::Duration;
use netcanv_protocol::relay;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{lookup_host, tcp, TcpStream};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::common::Fatal;

/// Runtime for managing active connections.
pub struct SocketSystem {
   runtime: tokio::runtime::Runtime,
   quitters: Mutex<Vec<SocketQuitter>>,
}

impl SocketSystem {
   /// Starts the socket system.
   pub fn new() -> Arc<Self> {
      Arc::new(Self {
         runtime: tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap(),
         quitters: Mutex::new(Vec::new()),
      })
   }

   /// Resolves the socket addresses the given hostname could refer to.
   async fn resolve_address_with_default_port(hostname: &str) -> anyhow::Result<Vec<SocketAddr>> {
      if let Ok(addresses) = lookup_host(hostname).await {
         Ok(addresses.collect())
      } else {
         Ok(lookup_host((hostname, relay::DEFAULT_PORT)).await?.collect())
      }
   }

   async fn connect_inner(self: Arc<Self>, hostname: String) -> anyhow::Result<Socket> {
      let addresses = Self::resolve_address_with_default_port(&hostname)
         .await
         .context("Could not resolve address. Are you sure the IP is correct?")?;
      log::info!("resolved addresses: {:?}", addresses);
      let stream = TcpStream::connect(addresses.as_slice()).await?;
      stream.set_nodelay(true)?;
      let (mut read_half, write_half) = stream.into_split();
      log::info!("connection established");

      let version = read_half.read_u32().await?;
      if version < relay::PROTOCOL_VERSION {
         anyhow::bail!("Relay version is too old. Try downgrading your client");
      } else if version > relay::PROTOCOL_VERSION {
         anyhow::bail!("Relay version is too new. Try updating your client");
      }
      log::debug!("version ok");

      log::debug!("starting receiver loop");
      let (recv_tx, recv_rx) = mpsc::unbounded_channel();
      let (recv_quit_tx, recv_quit_rx) = oneshot::channel();
      let recv_join_handle = self.runtime.spawn(async move {
         Socket::receiver_loop(read_half, recv_tx, recv_quit_rx).await.unwrap()
      });

      log::debug!("starting sender loop");
      let (send_tx, send_rx) = mpsc::unbounded_channel();
      let (send_quit_tx, send_quit_rx) = oneshot::channel();
      let send_join_handle = self.runtime.spawn(async move {
         Socket::sender_loop(write_half, send_rx, send_quit_rx).await.unwrap()
      });

      log::debug!("registering quitters");
      let mut quitters = self.quitters.lock().await;
      quitters.push(SocketQuitter {
         quit_send: send_quit_tx,
         quit_recv: recv_quit_tx,
         send_join_handle,
         recv_join_handle,
      });

      Ok(Socket {
         tx: send_tx,
         rx: recv_rx,
      })
   }

   /// Initiates a new connection to the relay at the given hostname (IP address or DNS domain).
   pub fn connect(self: Arc<Self>, hostname: String) -> oneshot::Receiver<anyhow::Result<Socket>> {
      log::info!("connecting to {}", hostname);
      let (socket_tx, socket_rx) = oneshot::channel();
      let self2 = Arc::clone(&self);
      self.runtime.spawn(async move {
         if let Err(_) = socket_tx.send(self2.connect_inner(hostname).await) {
            panic!("Could not send ready socket to receiver");
         }
      });
      socket_rx
   }
}

impl Drop for SocketSystem {
   fn drop(&mut self) {
      log::info!("cleaning up remaining sockets");
      self.runtime.block_on(async {
         let mut handles = self.quitters.lock().await;
         for handle in handles.drain(..) {
            handle.quit().await;
         }
      })
   }
}

pub struct Socket {
   tx: mpsc::UnboundedSender<relay::Packet>,
   rx: mpsc::UnboundedReceiver<relay::Packet>,
}

impl Socket {
   async fn read_packet(
      read_half: &mut tcp::OwnedReadHalf,
      len: usize,
      output: &mut mpsc::UnboundedSender<relay::Packet>,
   ) -> anyhow::Result<()> {
      if len > relay::MAX_PACKET_SIZE as usize {
         anyhow::bail!("Packet is too big");
      }
      let mut bytes = vec![0; len as usize];
      read_half.read_exact(&mut bytes).await?;
      let packet = bincode::deserialize(&bytes).context("Invalid packet")?;
      output.send(packet)?;
      Ok(())
   }

   async fn receiver_loop(
      mut read_half: tcp::OwnedReadHalf,
      mut output: mpsc::UnboundedSender<relay::Packet>,
      mut quit: oneshot::Receiver<Quit>,
   ) -> anyhow::Result<()> {
      loop {
         tokio::select! {
            biased;
            Ok(_) = &mut quit => {
               log::info!("receiver: received quit signal");
               break;
            },
            len = read_half.read_u32() => Self::read_packet(
               &mut read_half,
               len? as usize,
               &mut output
            ).await?,
            else => (),
         }
      }
      log::info!("receiver loop done");
      Ok(())
   }

   async fn write_packet(
      write_half: &mut tcp::OwnedWriteHalf,
      packet: relay::Packet,
   ) -> anyhow::Result<()> {
      let bytes = bincode::serialize(&packet)?;
      if bytes.len() > relay::MAX_PACKET_SIZE as usize {
         anyhow::bail!(
            "Cannot send packet that is bigger than {} bytes (got {})",
            relay::MAX_PACKET_SIZE,
            bytes.len(),
         );
      }
      write_half.write_u32(u32::try_from(bytes.len()).context("Packet is too big (wtf)")?).await?;
      write_half.write_all(&bytes).await?;
      write_half.flush().await?;
      Ok(())
   }

   async fn sender_loop(
      mut write_half: tcp::OwnedWriteHalf,
      mut input: mpsc::UnboundedReceiver<relay::Packet>,
      mut quit: oneshot::Receiver<Quit>,
   ) -> anyhow::Result<()> {
      loop {
         tokio::select! {
            biased;
            Ok(_) = &mut quit => {
               log::info!("sender: received quit signal");
               break;
            },
            packet = input.recv() => {
               if let Some(packet) = packet {
                  Self::write_packet(&mut write_half, packet).await?;
               } else {
                  break;
               }
            },
            else => (),
         }
      }
      log::info!("sender loop done");
      Ok(())
   }

   /// Sends a packet to the receiving end of the socket.
   pub fn send(&self, packet: relay::Packet) {
      catch!(self.tx.send(packet).context("The relay has disconnected"), as Fatal)
   }

   /// Receives packets from the sending end of the socket.
   pub fn recv(&mut self) -> Option<relay::Packet> {
      self.rx.try_recv().ok()
   }
}

struct Quit;

struct SocketQuitter {
   quit_send: oneshot::Sender<Quit>,
   quit_recv: oneshot::Sender<Quit>,
   send_join_handle: JoinHandle<()>,
   recv_join_handle: JoinHandle<()>,
}

impl SocketQuitter {
   async fn quit(self) {
      const QUIT_TIMEOUT: Duration = Duration::from_millis(250);
      let _ = self.quit_send.send(Quit);
      let _ = self.quit_recv.send(Quit);
      let _ = timeout(QUIT_TIMEOUT, self.send_join_handle).await;
      let _ = timeout(QUIT_TIMEOUT, self.recv_join_handle).await;
   }
}
