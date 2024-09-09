//! An abstraction for sockets, communicating over the global bus.

use std::cmp::Ordering;
use std::sync::Arc;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use netcanv_protocol::relay;
use nysa::global as bus;
use parking_lot::Mutex;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};
use url::Url;
use web_time::Duration;

use crate::common::{deserialize_bincode, serialize_bincode, Fatal};
use crate::Error;

/// Runtime for managing active connections.
pub struct SocketSystem {
   quitters: Mutex<Vec<SocketQuitter>>,
}

impl SocketSystem {
   /// Starts the socket system.
   pub fn new() -> Arc<Self> {
      Arc::new(Self {
         quitters: Mutex::new(Vec::new()),
      })
   }

   fn parse_url(url: &str) -> netcanv::Result<Url> {
      let url = if !url.starts_with("ws://") && !url.starts_with("wss://") {
         format!("wss://{}", url)
      } else {
         url.to_owned()
      };

      let url = Url::parse(&url).map_err(|_| Error::InvalidUrl)?;

      Ok(url)
   }

   async fn connect_inner(self: Arc<Self>, url: String) -> netcanv::Result<Socket> {
      let address = Self::parse_url(&url)?;
      let (stream, _) = connect_async(address).await?;
      let (sink, mut stream) = stream.split();
      tracing::info!("connection established");

      let version = stream.next().await.ok_or(Error::NoVersionPacket)?;

      let version = match version? {
         Message::Binary(version) => {
            let array: [u8; 4] = version.try_into().map_err(|_| Error::InvalidVersionPacket)?;
            u32::from_le_bytes(array)
         }
         _ => return Err(Error::InvalidVersionPacket),
      };

      match version.cmp(&relay::PROTOCOL_VERSION) {
         Ordering::Equal => (),
         Ordering::Less => return Err(Error::RelayIsTooOld),
         Ordering::Greater => return Err(Error::RelayIsTooNew),
      }

      tracing::debug!("version ok");

      let (quit_tx, _) = broadcast::channel(1);

      tracing::debug!("starting receiver loop");
      let (recv_tx, recv_rx) = mpsc::unbounded_channel();
      let (recv_quit_tx, recv_quit_rx) = (quit_tx.clone(), quit_tx.subscribe());
      let recv_join_handle = tokio::spawn(async move {
         if let Err(error) =
            Socket::receiver_loop(stream, recv_tx, recv_quit_tx, recv_quit_rx).await
         {
            tracing::error!("receiver loop error: {:?}", error);
         }
      });

      tracing::debug!("starting sender loop");
      let (send_tx, send_rx) = mpsc::unbounded_channel();
      let send_quit_rx = quit_tx.subscribe();
      let send_join_handle = tokio::spawn(async move {
         if let Err(error) = Socket::sender_loop(sink, send_rx, send_quit_rx).await {
            tracing::error!("sender loop error: {:?}", error);
         }
      });

      tracing::debug!("registering quitters");
      let mut quitters = self.quitters.lock();
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
   pub fn connect(self: Arc<Self>, hostname: String) -> oneshot::Receiver<netcanv::Result<Socket>> {
      tracing::info!("connecting to {}", hostname);
      let (socket_tx, socket_rx) = oneshot::channel();
      let self2 = Arc::clone(&self);
      tokio::spawn(async move {
         if socket_tx.send(self2.connect_inner(hostname).await).is_err() {
            panic!("Could not send ready socket to receiver");
         }
      });
      socket_rx
   }

   pub fn shutdown(self: Arc<Self>) {
      tracing::info!("shutting down socket system");
      tokio::spawn(async move {
         let mut handles = self.quitters.lock();
         for handle in handles.drain(..) {
            tokio::spawn(async move {
               handle.quit().await;
            });
         }
      });
   }
}

impl Drop for SocketSystem {
   fn drop(&mut self) {}
}

pub struct Socket {
   tx: mpsc::UnboundedSender<relay::Packet>,
   rx: mpsc::UnboundedReceiver<relay::Packet>,
}

type Stream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
type Sink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

impl Socket {
   /// Returns whether the connection was closed.
   async fn read_packet(
      message: tungstenite::Result<Message>,
      output: &mut mpsc::UnboundedSender<relay::Packet>,
      signal: &broadcast::Sender<Signal>,
   ) -> netcanv::Result<bool> {
      match message {
         Ok(Message::Binary(data)) => {
            if data.len() > relay::MAX_PACKET_SIZE as usize {
               return Err(Error::ReceivedPacketThatIsTooBig);
            }
            let packet = deserialize_bincode(&data)?;
            output.send(packet)?;
         }
         Ok(Message::Close(frame)) => {
            bus::push(Fatal(Error::RelayHasDisconnected));

            if let Some(frame) = frame {
               tracing::warn!(
                  "the relay has disconnected: {:?}, code: {}",
                  frame.reason,
                  frame.code
               );
            } else {
               tracing::warn!("the relay has disconnected (reason unknown)");
            }

            signal.send(Signal::Quit)?;

            return Ok(true);
         }
         Ok(Message::Ping(ping)) => {
            signal.send(Signal::SendPong(ping))?;
         }
         Err(e) => {
            use tokio_tungstenite::tungstenite::error::ProtocolError;
            use tokio_tungstenite::tungstenite::Error as WsError;
            match e {
               WsError::ConnectionClosed => return Ok(true),
               // Relay can force a closing handshake, and WebSockets requires a closing handshake.
               // If we do not get it, it means that relay has been closed and we have to close the session.
               WsError::AlreadyClosed
               | WsError::Protocol(ProtocolError::ResetWithoutClosingHandshake) => {
                  tracing::error!("the connection was closed without a closing handshake (relay probably crashed)");
                  bus::push(Fatal(Error::RelayHasDisconnected));
                  return Ok(true);
               }
               other => {
                  return Err(Error::WebSocket {
                     error: other.to_string(),
                  })
               }
            }
         }
         _ => tracing::info!("got unused message"),
      }

      Ok(false)
   }

   async fn receiver_loop(
      mut stream: Stream,
      mut output: mpsc::UnboundedSender<relay::Packet>,
      signal_tx: broadcast::Sender<Signal>,
      mut signal_rx: broadcast::Receiver<Signal>,
   ) -> netcanv::Result<()> {
      loop {
         tokio::select! {
            biased;
            Ok(signal) = signal_rx.recv() => {
               if let Signal::Quit = signal {
                  tracing::info!("receiver: received quit signal");
                  break;
               }
            },
            Some(message) = stream.next() => {
               if Self::read_packet(message, &mut output, &signal_tx).await? {
                  break
               }
            },
            else => (),
         }
      }
      tracing::info!("receiver loop done");
      Ok(())
   }

   async fn write_packet(sink: &mut Sink, packet: relay::Packet) -> netcanv::Result<()> {
      let bytes = serialize_bincode(&packet)?;
      if bytes.len() > relay::MAX_PACKET_SIZE as usize {
         return Err(Error::TriedToSendPacketThatIsTooBig {
            max: relay::MAX_PACKET_SIZE as usize,
            size: bytes.len(),
         });
      }
      u32::try_from(bytes.len()).map_err(|_| Error::TriedToSendPacketThatIsWayTooBig)?;

      sink.send(Message::Binary(bytes)).await?;
      Ok(())
   }

   async fn sender_loop(
      mut sink: Sink,
      mut input: mpsc::UnboundedReceiver<relay::Packet>,
      mut signal: broadcast::Receiver<Signal>,
   ) -> netcanv::Result<()> {
      loop {
         tokio::select! {
            biased;
            Ok(signal) = signal.recv() => {
               match signal {
                  Signal::Quit => {
                     tracing::info!("sender: received quit signal");
                     break;
                  }
                  Signal::SendPong(ping) => {
                     sink.send(Message::Pong(ping)).await?;
                  }
               }
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
      tracing::info!("sender loop done");
      Ok(())
   }

   /// Sends a packet to the receiving end of the socket.
   pub fn send(&self, packet: relay::Packet) {
      catch!(self.tx.send(packet).map_err(|_| Error::RelayHasDisconnected), as Fatal)
   }

   /// Receives packets from the sending end of the socket.
   pub fn recv(&mut self) -> Option<relay::Packet> {
      self.rx.try_recv().ok()
   }
}

#[derive(Clone, Debug)]
enum Signal {
   SendPong(Vec<u8>),
   Quit,
}

struct SocketQuitter {
   quit: broadcast::Sender<Signal>,
   send_join_handle: JoinHandle<()>,
   recv_join_handle: JoinHandle<()>,
}

impl SocketQuitter {
   async fn quit(self) {
      const QUIT_TIMEOUT: Duration = Duration::from_millis(250);
      let _ = self.quit.send(Signal::Quit);
      let _ = timeout(QUIT_TIMEOUT, self.send_join_handle).await;
      let _ = timeout(QUIT_TIMEOUT, self.recv_join_handle).await;
   }
}
