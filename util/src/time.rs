use std::fmt::Display;
use std::ops::{Add, AddAssign, Sub};

use derive_more::From;
use num_rational::{Rational32, Rational64};
use num_traits::identities::Zero;
use serde::{Deserialize, Serialize};

#[derive(From, Debug, Clone, Copy, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
pub struct MediaTime(Rational64);

impl MediaTime {
    pub fn new(numer: i64, denom: i64) -> Self {
        Rational64::new(numer, denom).into()
    }

    pub fn round_to_base(&self, base: i64) -> i64 {
        (self.0 * base).to_integer()
    }

    pub fn is_zero(&self) -> bool {
        self.0 == Rational64::zero()
    }

    pub fn zero() -> Self {
        MediaTime(Rational64::zero())
    }

    pub fn add_epoch(self, epoch: MediaTime) -> MediaTime {
        MediaTime(self.0 + epoch.0)
    }

    pub fn remove_epoch(self, epoch: MediaTime) -> MediaTime {
        MediaTime(self.0 - epoch.0)
    }

    pub fn as_rational(self) -> Rational64 {
        self.0
    }

    pub fn decimal(&self) -> impl Display {
        let micros = self.round_to_base(1_000_000);
        format!("{:.6}", micros as f64 / 1_000_000.0)
    }
}

impl Add<MediaDuration> for MediaTime {
    type Output = MediaTime;

    fn add(self, rhs: MediaDuration) -> MediaTime {
        MediaTime(self.0 + rhs.0)
    }
}

impl AddAssign<MediaDuration> for MediaTime {
    fn add_assign(&mut self, rhs: MediaDuration) {
        self.0 += rhs.0
    }
}

impl Sub<MediaDuration> for MediaTime {
    type Output = MediaTime;

    fn sub(self, rhs: MediaDuration) -> MediaTime {
        MediaTime(self.0 - rhs.0)
    }
}

impl Sub<MediaTime> for MediaTime {
    type Output = MediaDuration;

    fn sub(self, rhs: MediaTime) -> MediaDuration {
        MediaDuration(self.0 - rhs.0)
    }
}

#[derive(From, Debug, Clone, Copy, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
pub struct MediaDuration(Rational64);

impl MediaDuration {
    pub fn new(numer: i64, denom: i64) -> Self {
        Rational64::new(numer, denom).into()
    }

    pub fn round_to_base(&self, base: i64) -> i64 {
        (self.0 * base).to_integer()
    }

    pub fn is_zero(&self) -> bool {
        self.0 == Rational64::zero()
    }

    pub fn zero() -> Self {
        MediaDuration(Rational64::zero())
    }

    pub fn as_rational(self) -> Rational64 {
        self.0
    }

    pub fn decimal(&self) -> impl Display {
        let micros = self.round_to_base(1_000_000);
        format!("{:.6}", micros as f64 / 1_000_000.0)
    }
}

impl Add<MediaDuration> for MediaDuration {
    type Output = MediaDuration;

    fn add(self, rhs: MediaDuration) -> MediaDuration {
        MediaDuration(self.0 + rhs.0)
    }
}

/// This is a Rational32 for easy interop with ffmpeg's time base
#[derive(From, Debug, Clone, Copy, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
pub struct TimeBase(Rational32);

impl TimeBase {
    pub fn new(numer: i32, denom: i32) -> Self {
        Rational32::new(numer, denom).into()
    }

    fn scale(&self, units: i64) -> Rational64 {
        let numer = i64::from(*self.0.numer());
        let denom = i64::from(*self.0.denom());
        Rational64::new(units * numer, denom)
    }

    fn unscale(&self, units: Rational64) -> i64 {
        (units / rat32_to_64(self.0)).to_integer()
    }

    pub fn scale_timestamp(&self, timestamp: i64) -> MediaTime {
        MediaTime(self.scale(timestamp))
    }

    pub fn unscale_timestamp(&self, time: MediaTime) -> i64 {
        self.unscale(time.0)
    }

    pub fn scale_duration(&self, duration: i64) -> MediaDuration {
        MediaDuration(self.scale(duration))
    }

    pub fn as_rational(self) -> Rational32 {
        self.0
    }

    pub fn display(&self) -> impl Display {
        self.0
    }
}

fn rat32_to_64(rat: Rational32) -> Rational64 {
    Rational64::new((*rat.numer()).into(), (*rat.denom()).into())
}
