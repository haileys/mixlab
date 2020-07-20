use derive_more::From;
use num_rational::Rational64;

#[derive(From)]
pub struct StreamTime(Rational64);

impl StreamTime {
    pub fn new(numer: i64, denom: i64) -> Self {
        Rational64::new(numer, denom).into()
    }
}

#[derive(From)]
pub struct StreamDuration(Rational64);

impl StreamDuration {
    pub fn new(numer: i64, denom: i64) -> Self {
        Rational64::new(numer, denom).into()
    }
}
