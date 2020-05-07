use std::thread;
use std::time::{Duration, Instant};

use crate::engine::SAMPLE_RATE;

pub struct AudioThrottle {
    started: Option<Instant>,
    samples_sent: u64,
}

impl AudioThrottle {
    pub fn new() -> AudioThrottle {
        AudioThrottle {
            started: None,
            samples_sent: 0,
        }
    }

    pub fn send_samples(&mut self, sample_count: usize) {
        let started = *self.started.get_or_insert_with(Instant::now);

        let elapsed = Duration::from_micros((self.samples_sent * 1_000_000) / SAMPLE_RATE as u64);
        let sleep_until = started + elapsed;
        let now = Instant::now();

        if now < sleep_until {
            thread::sleep(sleep_until - now);
        }

        self.samples_sent += sample_count as u64;
    }
}
