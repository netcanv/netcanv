//! An abstraction for sockets, communicating over the global bus.

use std::cmp::Ordering;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use instant::Duration;
use netcanv_protocol::relay;
use nysa::global as bus;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};

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
   async fn resolve_address_with_default_port(url: &str) -> anyhow::Result<url::Url> {
      let url = if !url.starts_with("ws://") && !url.starts_with("wss://") {
         format!("wss://{}", url)
      } else {
         url.to_owned()
      };

      let mut url = url::Url::parse(&url)?;

      if url.port().is_none() {
         // Url::set_port on Error does nothing, so it is fine to ignore it
         let _ = url.set_port(Some(relay::DEFAULT_PORT));
      }

      Ok(url)
   }

   async fn connect_inner(self: Arc<Self>, hostname: String) -> anyhow::Result<Socket> {
      let address = Self::resolve_address_with_default_port(&hostname)
         .await
         .context("Could not resolve address. Are you sure the address is correct?")?;
      let (stream, _) = connect_async(address).await?;
      let (sink, mut stream) = stream.split();
      log::info!("connection established");

      let version =
         stream.next().await.ok_or_else(|| anyhow::anyhow!("Didn't receive version packet."))?;

      let version = match version? {
         Message::Binary(version) => {
            let array: [u8; 4] = version
               .try_into()
               .map_err(|_| anyhow::anyhow!("The relay sent an invalid version packet."))?;
            u32::from_le_bytes(array)
         }
         _ => anyhow::bail!("The relay sent an invalid packet."),
      };

      match version.cmp(&relay::PROTOCOL_VERSION) {
         Ordering::Equal => (),
         Ordering::Less => anyhow::bail!("Relay version is too old. Try downgrading your client"),
         Ordering::Greater => anyhow::bail!("Relay version is too new. Try updating your client"),
      }

      log::debug!("version ok");

      let (quit_tx, _) = broadcast::channel(1);

      log::debug!("starting receiver loop");
      let (recv_tx, recv_rx) = mpsc::unbounded_channel();
      let (recv_quit_tx, recv_quit_rx) = (quit_tx.clone(), quit_tx.subscribe());
      let recv_join_handle = self.runtime.spawn(async move {
         if let Err(error) =
            Socket::receiver_loop(stream, recv_tx, recv_quit_tx, recv_quit_rx).await
         {
            log::error!("receiver loop erro: {:?}", error);
         }
      });

      log::debug!("starting sender loop");
      let (send_tx, send_rx) = mpsc::unbounded_channel();
      let send_quit_rx = quit_tx.subscribe();
      let send_join_handle = self.runtime.spawn(async move {
         if let Err(error) = Socket::sender_loop(sink, send_rx, send_quit_rx).await {
            log::error!("sender loop error: {:?}", error);
         }
      });

      log::debug!("registering quitters");
      let mut quitters = self.quitters.lock().await;
      quitters.push(SocketQuitter {
         quit: quit_tx,
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
         if socket_tx.send(self2.connect_inner(hostname).await).is_err() {
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

type Stream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
type Sink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

impl Socket {
   async fn read_packet(
      message: tungstenite::Result<Message>,
      output: &mut mpsc::UnboundedSender<relay::Packet>,
      quit: &broadcast::Sender<Quit>,
   ) -> anyhow::Result<Option<()>> {
      match message {
         Ok(Message::Binary(data)) => {
            if data.len() > relay::MAX_PACKET_SIZE as usize {
               anyhow::bail!("Packet is too big");
            }
            let packet = bincode::deserialize(&data).context("Invalid packet")?;
            output.send(packet)?;
         }
         Ok(Message::Close(frame)) => {
            bus::push(Fatal(anyhow::anyhow!("The relay has been disconnected.")));

            if let Some(frame) = frame {
               log::warn!(
                  "the relay has been disconnected: {:?}, code: {}",
                  frame.reason,
                  frame.code
               );
            } else {
               log::warn!("the relay has been disconnected (reason unknown)");
            }

            quit.send(Quit)?;

            return Ok(None);
         }
         Err(e) => {
            use tokio_tungstenite::tungstenite::error::ProtocolError;
            use tokio_tungstenite::tungstenite::Error::*;
            match e {
               ConnectionClosed => return Ok(None),
               // Relay can force a closing handshake, and WebSockets requires a closing handshake.
               // If we do not get it, it means that relay has been closed and we have to close the session.
               AlreadyClosed | Protocol(ProtocolError::ResetWithoutClosingHandshake) => {
                  log::error!("the connection was closed without a closing handshake (relay probably crashed)");
                  bus::push(Fatal(anyhow::anyhow!("The relay has been disconnected.")));
                  return Ok(None);
               }
               _ => anyhow::bail!(e),
            }
         }
         _ => log::info!("got unused message"),
      }

      Ok(Some(()))
   }

   async fn receiver_loop(
      mut stream: Stream,
      mut output: mpsc::UnboundedSender<relay::Packet>,
      quit_tx: broadcast::Sender<Quit>,
      mut quit_rx: broadcast::Receiver<Quit>,
   ) -> anyhow::Result<()> {
      loop {
         tokio::select! {
            biased;
            Ok(_) = quit_rx.recv() => {
               log::info!("receiver: received quit signal");
               break;
            },
            Some(message) = stream.next() => if Self::read_packet(message, &mut output, &quit_tx).await?.is_none() {
               break
            },
            else => (),
         }
      }
      log::info!("receiver loop done");
      Ok(())
   }

   async fn write_packet(sink: &mut Sink, packet: relay::Packet) -> anyhow::Result<()> {
      let bytes = bincode::serialize(&packet)?;
      if bytes.len() > relay::MAX_PACKET_SIZE as usize {
         anyhow::bail!(
            "Cannot send packet that is bigger than {} bytes (got {})",
            relay::MAX_PACKET_SIZE,
            bytes.len(),
         );
      }
      u32::try_from(bytes.len()).context("Packet is too big (wtf)")?;

      sink.send(Message::Binary(bytes)).await?;
      Ok(())
   }

   async fn sender_loop(
      mut sink: Sink,
      mut input: mpsc::UnboundedReceiver<relay::Packet>,
      mut quit: broadcast::Receiver<Quit>,
   ) -> anyhow::Result<()> {
      loop {
         tokio::select! {
            biased;
            Ok(_) = quit.recv() => {
               log::info!("sender: received quit signal");
               break;
            },
            packet = input.recv() => {
               if let Some(packet) = packet {
                  Self::write_packet(&mut sink, packet).await?;
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

#[derive(Clone, Debug)]
struct Quit;

struct SocketQuitter {
   quit: broadcast::Sender<Quit>,
   send_join_handle: JoinHandle<()>,
   recv_join_handle: JoinHandle<()>,
}

impl SocketQuitter {
   async fn quit(self) {
      const QUIT_TIMEOUT: Duration = Duration::from_millis(250);
      let _ = self.quit.send(Quit);
      let _ = timeout(QUIT_TIMEOUT, self.send_join_handle).await;
      let _ = timeout(QUIT_TIMEOUT, self.recv_join_handle).await;
   }
}
