use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ringbuf::{RingBuffer, Producer, Consumer};

use mixlab_util::time::MediaTime;

use crate::util::Sequence;
use crate::video;

#[derive(Clone)]
pub struct Registry {
    inner: Arc<Mutex<RegistryInner>>,
}

struct RegistryInner {
    channels: HashMap<String, Source>,
}

struct Source {
    shared: Arc<SourceShared>,
    seq: Sequence,
    tx: Option<TxPair>,
}

struct TxPair {
    audio: Producer<Frame<AudioData>>,
    video: Producer<Frame<VideoData>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceId(NonZeroUsize);

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
    source_id: SourceId,
    // this is, regrettably, an Option because we need to take the tx pair
    // and put it back in the mountpoints table on drop:
    tx: Option<TxPair>,
}

pub type AudioData = Vec<i16>;
pub type VideoData = Arc<video::Frame>;

#[derive(Debug)]
pub struct Frame<T> {
    pub source_id: SourceId,
    pub source_time: MediaTime,
    pub data: T,
}

pub struct SourceRecv {
    registry: Registry,
    shared: Arc<SourceShared>,
    audio_rx: Consumer<Frame<AudioData>>,
    video_rx: Consumer<Frame<VideoData>>,
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

        let (audio_tx, audio_rx) = RingBuffer::<Frame<AudioData>>::new(65536).split();
        let (video_tx, video_rx) = RingBuffer::<Frame<VideoData>>::new(65536).split();

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
            seq: Sequence::new(),
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

        let source_id = SourceId(source.seq.next());

        let tx = source.tx.take().ok_or(ConnectError::AlreadyConnected)?;

        Ok(SourceSend {
            registry: self.clone(),
            shared: source.shared.clone(),
            tx: Some(tx),
            source_id,
        })
    }
}

impl SourceSend {
    pub fn connected(&self) -> bool {
        self.shared.recv_online.load(Ordering::Relaxed)
    }

    pub fn write_audio(&mut self, timestamp: MediaTime, data: AudioData) -> Result<(), ()> {
        if self.connected() {
            // tx is always Some for a valid (non-dropped) SourceSend:
            let tx = self.tx.as_mut().unwrap();

            let frame = Frame {
                source_id: self.source_id,
                source_time: timestamp,
                data,
            };

            tx.audio.push(frame).map_err(|_| ())
        } else {
            Err(())
        }
    }

    pub fn write_video(&mut self, timestamp: MediaTime, data: VideoData) -> Result<(), ()> {
        if self.connected() {
            // tx is always Some for a valid (non-dropped) SourceSend:
            let tx = self.tx.as_mut().unwrap();

            let frame = Frame {
                source_id: self.source_id,
                source_time: timestamp,
                data,
            };

            tx.video.push(frame).map_err(|_| ())
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

    pub fn read_audio(&mut self) -> Option<Frame<AudioData>> {
        self.audio_rx.pop()
    }

    pub fn read_video(&mut self) -> Option<Frame<VideoData>> {
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
