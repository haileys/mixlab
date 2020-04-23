use std::sync::Arc;

use num_rational::Rational64;
use mixlab_codec::avc::AvcFrame;

#[derive(Debug)]
pub struct Frame {
    pub specific: AvcFrame,

    // frame duration in fractional seconds, possibly an estimate if frame
    // duration information is not available:
    pub duration_hint: Rational64,

    // points to any key frame that may be necessary to decode this frame
    pub key_frame: Option<Arc<Frame>>,
}
