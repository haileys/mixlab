use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ringbuf::{RingBuffer, Producer, Consumer};

use crate::engine::Sample;

lazy_static::lazy_static! {
    static ref MOUNTPOINTS: Mutex<HashMap<String, Source>> = Mutex::new(HashMap::new());
}

struct Source {
    shared: Arc<SourceShared>,
    tx: Option<Producer<Sample>>,
}

#[derive(Debug)]
struct SourceShared {
    mountpoint: String,
    recv_online: AtomicBool,
}

#[derive(Debug)]
pub enum ListenError {
    AlreadyInUse,
}

pub fn listen(mountpoint: &str) -> Result<SourceRecv, ListenError> {
    let mut mountpoints = MOUNTPOINTS.lock()
        .expect("mountpoints lock");

    if mountpoints.contains_key(mountpoint) {
        return Err(ListenError::AlreadyInUse);
    }

    let (tx, rx) = RingBuffer::<Sample>::new(65536).split();

    let shared = Arc::new(SourceShared {
        mountpoint: mountpoint.to_owned(),
        recv_online: AtomicBool::new(true),
    });

    let recv = SourceRecv {
        shared: shared.clone(),
        rx,
    };

    let source = Source {
        shared: shared.clone(),
        tx: Some(tx),
    };

    mountpoints.insert(mountpoint.to_owned(), source);

    Ok(recv)
}

#[derive(Debug)]
pub enum ConnectError {
    NoMountpoint,
    AlreadyConnected,
}

pub fn connect(mountpoint: &str) -> Result<SourceSend, ConnectError> {
    let mut mountpoints = MOUNTPOINTS.lock()
        .expect("mountpoints lock");

    let source = match mountpoints.get_mut(mountpoint) {
        None => { return Err(ConnectError::NoMountpoint); }
        Some(source) => source,
    };

    let tx = match source.tx.take() {
        None => { return Err(ConnectError::AlreadyConnected); }
        Some(tx) => tx,
    };

    Ok(SourceSend {
        shared: source.shared.clone(),
        tx: Some(tx),
    })
}

pub struct SourceSend {
    shared: Arc<SourceShared>,
    // this is, regrettably, an Option because we need to take the producer
    // and put it back in the mountpoints table on drop:
    tx: Option<Producer<Sample>>,
}

impl SourceSend {
    pub fn connected(&self) -> bool {
        self.shared.recv_online.load(Ordering::Relaxed)
    }

    pub fn write(&mut self, data: &[Sample]) -> Result<usize, ()> {
        if self.connected() {
            if let Some(tx) = &mut self.tx {
                Ok(tx.push_slice(data))
            } else {
                // tx is always Some for a valid (non-dropped) SourceSend
                unreachable!()
            }
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
        let mut mountpoints = MOUNTPOINTS.lock()
            .expect("mountpoints lock");

        match mountpoints.get_mut(&self.shared.mountpoint) {
            None => {
                // receiver has disconnected, there is nothing to do
            }
            Some(mountpoint) => {
                mountpoint.tx = self.tx.take();
            }
        }
    }
}

pub struct SourceRecv {
    shared: Arc<SourceShared>,
    rx: Consumer<Sample>,
}

impl Debug for SourceRecv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SourceRecv {{ shared: {:?}, .. }}", self.shared)
    }
}

impl SourceRecv {
    pub fn mountpoint(&self) -> &str {
        &self.shared.mountpoint
    }

    pub fn read(&mut self, data: &mut [Sample]) -> usize {
        self.rx.pop_slice(data)
    }
}

impl Drop for SourceRecv {
    fn drop(&mut self) {
        MOUNTPOINTS.lock()
            .expect("mountpoints lock")
            .remove(&self.shared.mountpoint);

        self.shared.recv_online.store(false, Ordering::Relaxed);
    }
}
