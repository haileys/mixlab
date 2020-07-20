use derive_more::From;
use num_rational::Rational64;
use num_traits::identities::Zero;
use serde::{Deserialize, Serialize};

#[derive(From, Debug, Clone, Deserialize, Serialize)]
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
}

#[derive(From, Debug, Clone, Deserialize, Serialize)]
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
}
