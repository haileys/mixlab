use std::convert::TryFrom;
use std::thread;
use std::time::{Duration, Instant};

use mixlab_util::time::MediaTime;

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

pub struct MediaThrottle {
    started: Option<Instant>,
}

impl MediaThrottle {
    pub fn new() -> MediaThrottle {
        MediaThrottle {
            started: None,
        }
    }

    pub fn wait_until(&mut self, time: MediaTime) {
        let started = *self.started.get_or_insert_with(Instant::now);

        let elapsed = Duration::from_micros(
            u64::try_from(time.round_to_base(1_000_000))
                .expect("elapsed is non-negative"));

        let sleep_until = started + elapsed;

        let now = Instant::now();

        if now < sleep_until {
            thread::sleep(sleep_until - now);
        }
    }
}
