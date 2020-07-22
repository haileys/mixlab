use std::ops::{Add, AddAssign, Sub};

use derive_more::From;
use num_rational::Rational64;
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
}

impl Add<MediaDuration> for MediaDuration {
    type Output = MediaDuration;

    fn add(self, rhs: MediaDuration) -> MediaDuration {
        MediaDuration(self.0 + rhs.0)
    }
}
