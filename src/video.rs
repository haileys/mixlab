use std::sync::Arc;

use num_rational::Rational64;

use crate::codec::avc::AvcFrame;

#[derive(Debug)]
pub struct Frame {
    pub specific: Arc<AvcFrame>,

    // frame duration in fractional seconds, possibly an estimate if frame
    // duration information is not available:
    pub duration_hint: Rational64,
}
