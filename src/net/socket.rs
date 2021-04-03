// socket abstraction.

use std::net::{ToSocketAddrs, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

struct Finished;
struct Abort;
struct Tick;

// P is the packet type
pub struct Remote<P: Serialize + DeserializeOwned + Send + 'static> {
    rx: Receiver<P>,
    tx: Sender<P>,
    finished: Receiver<Finished>,
    abort: Sender<Abort>,
    tick: Sender<Tick>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] bincode::Error),
    #[error("Error while sending data across threads")]
    ThreadSend,
    #[error("Error while receiving data from the network thread")]
    ThreadRecv,
}

impl<P: Serialize + DeserializeOwned + Send + core::fmt::Debug + 'static> Remote<P> {

    pub fn new(addr: impl ToSocketAddrs) -> Result<Self, Error> {
        let stream = Arc::new(TcpStream::connect(addr)?);
        // we don't actually want fully non-blocking I/O so as not to spinlock the network thread
        const TIMEOUT: u64 = 10;
        stream.set_write_timeout(Some(Duration::from_millis(TIMEOUT)))?;
        stream.set_read_timeout(Some(Duration::from_millis(TIMEOUT)))?;
        stream.set_nodelay(true)?;

        let (to_thread, from_main) = crossbeam_channel::unbounded();
        let (to_main, from_thread) = crossbeam_channel::unbounded();
        let (tx_finished, rx_finished) = crossbeam_channel::unbounded();
        let (tx_abort, rx_abort) = crossbeam_channel::unbounded();
        let (tx_tick, rx_tick) = crossbeam_channel::unbounded();

        let _send_thread = std::thread::spawn(move || -> Result<(), Error> {
            'outer: loop {
                macro_rules! ok_or_break {
                    ($e:expr) => {
                        match $e {
                            Err(_) => { break 'outer },
                            _ => (),
                        }
                    };
                }

                // wait until the main thread calls tick()
                let _ = rx_tick.recv();
                // abort if requested
                if let Ok(_) = rx_abort.try_recv() {
                    break;
                }

                // otherwise send/receive packets
                use bincode::ErrorKind as BincodeError;
                use std::io::ErrorKind as IoError;
                while let Ok(packet) = from_main.try_recv() {
                    loop {
                        if let Err(error) = bincode::serialize_into(&*stream, &packet) {
                            match *error {
                                BincodeError::Io(error) if error.kind() == IoError::WouldBlock => {
                                    // repeat until it stops blocking
                                    continue;
                                },
                                _ => (),
                            }
                        }
                        break;
                    }
                }
                loop {
                    match bincode::deserialize_from::<_, P>(&*stream) {
                        // finish the thread if the channel is disconnected
                        Ok(packet) => { ok_or_break!(to_main.send(packet)); },
                        _ => (),
                    }
                    break;
                }
            }

            tx_finished.send(Finished).map_err(|_| Error::ThreadSend)
        });

        Ok(Self {
            rx: from_thread,
            tx: to_thread,
            finished: rx_finished,
            abort: tx_abort,
            tick: tx_tick,
        })
    }

    pub fn send(&self, packet: P) -> Result<(), Error> {
        self.tx.send(packet).map_err(|_| Error::ThreadSend)
    }

    pub fn recv(&self) -> Result<P, Error> {
        Ok(self.rx.recv().map_err(|_| Error::ThreadRecv)?)
    }

    pub fn try_recv(&self) -> Option<P> {
        self.rx.try_recv().ok()
    }

    pub fn tick(&self) -> Result<bool, Error> {
        self.tick.send(Tick).map_err(|_| Error::ThreadSend)?;
        match self.finished.try_recv() {
            Ok(_) => Ok(true),
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => Err(Error::ThreadRecv),
        }
    }

}

impl<P: Serialize + DeserializeOwned + Send> Drop for Remote<P> {

    fn drop(&mut self) {
        // intentionally ignore the result:
        // if the thread has already finished, this will fail with an error, because the receiving end has already
        // disconnected.
        let _ = self.abort.send(Abort);
    }

}
