use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ringbuf::{RingBuffer, Producer, Consumer};

use crate::codec::avc::AvcFrame;
use crate::engine::Sample;

#[derive(Clone)]
pub struct Registry {
    inner: Arc<Mutex<RegistryInner>>,
}

struct RegistryInner {
    channels: HashMap<String, Source>,
}

pub struct Source {
    shared: Arc<SourceShared>,
    tx: Option<TxPair>,
}

struct TxPair {
    audio: Producer<Sample>,
    video: Producer<AvcFrame>,
}

#[derive(Debug)]
pub struct SourceShared {
    channel_name: String,
    recv_online: AtomicBool,
}

#[derive(Debug)]
pub enum ListenError {
    AlreadyInUse,
}

#[derive(Debug)]
pub enum ConnectError {
    NoMountpoint,
    AlreadyConnected,
}

pub struct SourceSend {
    registry: Registry,
    shared: Arc<SourceShared>,
    // this is, regrettably, an Option because we need to take the tx pair
    // and put it back in the mountpoints table on drop:
    tx: Option<TxPair>,
}

pub struct SourceRecv {
    registry: Registry,
    shared: Arc<SourceShared>,
    audio_rx: Consumer<Sample>,
    video_rx: Consumer<AvcFrame>,
}

impl Registry {
    pub fn new() -> Self {
        let inner = RegistryInner {
            channels: HashMap::new(),
        };

        Registry { inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn listen(&self, channel_name: &str) -> Result<SourceRecv, ListenError> {
        let mut registry = self.inner.lock()
            .expect("registry lock");

        if registry.channels.contains_key(channel_name) {
            return Err(ListenError::AlreadyInUse);
        }

        let (audio_tx, audio_rx) = RingBuffer::<Sample>::new(65536).split();
        let (video_tx, video_rx) = RingBuffer::<AvcFrame>::new(128).split();

        let shared = Arc::new(SourceShared {
            channel_name: channel_name.to_owned(),
            recv_online: AtomicBool::new(true),
        });

        let recv = SourceRecv {
            registry: self.clone(),
            shared: shared.clone(),
            audio_rx,
            video_rx,
        };

        let source = Source {
            shared: shared.clone(),
            tx: Some(TxPair {
                audio: audio_tx,
                video: video_tx,
            }),
        };

        registry.channels.insert(channel_name.to_owned(), source);

        Ok(recv)
    }

    pub fn connect(&self, channel_name: &str) -> Result<SourceSend, ConnectError> {
        let mut registry = self.inner.lock()
            .expect("registry lock");

        let source = match registry.channels.get_mut(channel_name) {
            None => { return Err(ConnectError::NoMountpoint); }
            Some(source) => source,
        };

        let tx = source.tx.take().ok_or(ConnectError::AlreadyConnected)?;

        Ok(SourceSend {
            registry: self.clone(),
            shared: source.shared.clone(),
            tx: Some(tx),
        })
    }
}

impl SourceSend {
    pub fn connected(&self) -> bool {
        self.shared.recv_online.load(Ordering::Relaxed)
    }

    pub fn write_audio(&mut self, data: &[Sample]) -> Result<(), ()> {
        if self.connected() {
            // tx is always Some for a valid (non-dropped) SourceSend:
            let tx = self.tx.as_mut().unwrap();

            // we intentionally ignore the return value of write indicating
            // how many samples were written to the ring buffer. if it's
            // ever less than the number of samples on hand, the receive
            // side is suffering from serious lag and the best we can do
            // is drop the data

            let _ = tx.audio.push_slice(data);

            Ok(())
        } else {
            Err(())
        }
    }

    pub fn write_video(&mut self, data: AvcFrame) -> Result<(), ()> {
        if self.connected() {
            // tx is always Some for a valid (non-dropped) SourceSend:
            let tx = self.tx.as_mut().unwrap();

            let _ = tx.video.push(data);

            Ok(())
        } else {
            Err(())
        }
    }
}

impl Debug for SourceSend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SourceSend {{ shared: {:?}, .. }}", self.shared)
    }
}

impl Drop for SourceSend {
    fn drop(&mut self) {
        let mut registry = self.registry.inner.lock()
            .expect("registry lock");

        match registry.channels.get_mut(&self.shared.channel_name) {
            None => {
                // receiver has disconnected, there is nothing to do
            }
            Some(channel) => {
                channel.tx = self.tx.take();
            }
        }
    }
}

impl Debug for SourceRecv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SourceRecv {{ shared: {:?}, .. }}", self.shared)
    }
}

impl SourceRecv {
    pub fn channel_name(&self) -> &str {
        &self.shared.channel_name
    }

    pub fn read_audio(&mut self, data: &mut [Sample]) -> usize {
        self.audio_rx.pop_slice(data)
    }

    pub fn read_video(&mut self) -> Option<AvcFrame> {
        self.video_rx.pop()
    }
}

impl Drop for SourceRecv {
    fn drop(&mut self) {
        self.registry.inner.lock()
            .expect("registry lock")
            .channels
            .remove(&self.shared.channel_name);

        self.shared.recv_online.store(false, Ordering::Relaxed);
    }
}
