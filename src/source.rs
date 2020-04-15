use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Instant, Duration};

use ringbuf::{RingBuffer, Producer, Consumer};

use crate::engine::{Sample, SAMPLE_RATE};

#[derive(Clone)]
pub struct Registry {
    inner: Arc<Mutex<RegistryInner>>,
}

struct RegistryInner {
    channels: HashMap<String, Source>,
}

pub struct Source {
    shared: Arc<SourceShared>,
    tx: Option<Producer<Sample>>,
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
    // this is, regrettably, an Option because we need to take the producer
    // and put it back in the mountpoints table on drop:
    tx: Option<Producer<Sample>>,
    // throttling data:
    started: Option<Instant>,
    samples_sent: u64,
}

pub struct SourceRecv {
    registry: Registry,
    shared: Arc<SourceShared>,
    rx: Consumer<Sample>,
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

        let (tx, rx) = RingBuffer::<Sample>::new(65536).split();

        let shared = Arc::new(SourceShared {
            channel_name: channel_name.to_owned(),
            recv_online: AtomicBool::new(true),
        });

        let recv = SourceRecv {
            registry: self.clone(),
            shared: shared.clone(),
            rx,
        };

        let source = Source {
            shared: shared.clone(),
            tx: Some(tx),
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

        let tx = match source.tx.take() {
            None => { return Err(ConnectError::AlreadyConnected); }
            Some(tx) => tx,
        };

        Ok(SourceSend {
            registry: self.clone(),
            shared: source.shared.clone(),
            tx: Some(tx),
            started: None,
            samples_sent: 0,
        })
    }
}

impl SourceSend {
    pub fn connected(&self) -> bool {
        self.shared.recv_online.load(Ordering::Relaxed)
    }

    pub fn write(&mut self, data: &[Sample]) -> Result<(), ()> {
        if self.connected() {
            let started = *self.started.get_or_insert_with(Instant::now);

            // tx is always Some for a valid (non-dropped) SourceSend:
            let tx = self.tx.as_mut().unwrap();

            // we intentionally ignore the return value of write indicating
            // how many samples were written to the ring buffer. if it's
            // ever less than the number of samples on hand, the receive
            // side is suffering from serious lag and the best we can do
            // is drop the data

            let _ = tx.push_slice(data);

            // throttle ourselves according to how long these samples should
            // take to play through. this ensures that fast clients don't
            // fill the ring buffer up on us

            let elapsed = Duration::from_micros((self.samples_sent * 1_000_000) / SAMPLE_RATE as u64);
            let sleep_until = started + elapsed;
            let now = Instant::now();

            if now < sleep_until {
                thread::sleep(sleep_until - now);
            }

            // assume stereo:
            let sample_count = data.len() / 2;

            self.samples_sent += sample_count as u64;

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

    pub fn read(&mut self, data: &mut [Sample]) -> usize {
        self.rx.pop_slice(data)
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
