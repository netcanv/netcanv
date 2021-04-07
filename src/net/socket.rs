// socket abstraction.

use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

struct Finished;
struct Abort;

struct ControllableThread {
    finished: Receiver<Finished>,
    abort: Sender<Abort>,
}

impl ControllableThread {
    fn new<F, T>(name: &'static str, f: F) -> ControllableThread
    where
        F: FnOnce(Receiver<Abort>) -> Result<(), T> + Send + 'static,
        T: std::fmt::Display,
    {
        let (tx_finished, rx_finished) = crossbeam_channel::unbounded();
        let (tx_abort, rx_abort) = crossbeam_channel::unbounded();

        let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
            match f(rx_abort) {
                Err(error) => eprintln!("thread '{}' returned with error: {}", name, error),
                _ => (),
            }
            let _ = tx_finished.send(Finished);
        });

        ControllableThread {
            finished: rx_finished,
            abort: tx_abort,
        }
    }

    fn tick(&self) -> Result<bool, Error> {
        match self.finished.try_recv() {
            Ok(_) => Ok(true),
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => Err(Error::ThreadRecv),
        }
    }

    fn abort(&self) -> Result<(), Error> {
        self.abort.send(Abort).map_err(|_| Error::ThreadSend)
    }
}

// P is the packet type
pub struct Remote<P: Serialize + DeserializeOwned + Send + 'static> {
    rx: Receiver<P>,
    tx: Sender<P>,
    send: ControllableThread,
    recv: ControllableThread,
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
        let stream_arc = Arc::new(TcpStream::connect(addr)?);
        stream_arc.set_nodelay(true)?;

        let (to_thread, from_main) = crossbeam_channel::unbounded();
        let (to_main, from_thread) = crossbeam_channel::unbounded();

        let stream = stream_arc.clone();
        let send = ControllableThread::new("network send thread", move |abort| -> Result<(), Error> {
            loop {
                if let Ok(_) | Err(TryRecvError::Disconnected) = abort.try_recv() {
                    break
                }
                while let Ok(packet) = from_main.recv() {
                    bincode::serialize_into(&*stream, &packet)?;
                }
            }
            Ok(())
        });

        let stream = stream_arc.clone();
        let recv = ControllableThread::new("network recv thread", move |abort| -> Result<(), Error> {
            loop {
                if let Ok(_) | Err(TryRecvError::Disconnected) = abort.try_recv() {
                    break
                }
                let packet = bincode::deserialize_from(&*stream)?;
                to_main.send(packet).map_err(|_| Error::ThreadSend)?;
            }
            Ok(())
        });

        Ok(Self {
            rx: from_thread,
            tx: to_thread,
            send,
            recv,
        })
    }

    pub fn send(&self, packet: P) -> Result<(), Error> {
        self.tx.send(packet).map_err(|_| Error::ThreadSend)
    }

    pub fn try_recv(&self) -> Option<P> {
        self.rx.try_recv().ok()
    }

    pub fn tick(&self) -> Result<bool, Error> {
        Ok(self.send.tick()? && self.recv.tick()?)
    }
}

impl<P: Serialize + DeserializeOwned + Send> Drop for Remote<P> {
    fn drop(&mut self) {
        // intentionally ignore the result:
        // if the thread has already finished, this will fail with an error, because the receiving end has
        // already disconnected.
        let _ = self.send.abort();
        let _ = self.recv.abort();
    }
}
