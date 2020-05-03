use num_rational::Rational64;
use mixlab_codec::ffmpeg::AvFrame;

#[derive(Debug)]
pub struct Frame {
    pub decoded: AvFrame,

    // frame duration in fractional seconds, possibly an estimate if frame
    // duration information is not available:
    pub duration_hint: Rational64,
}

impl Frame {
    pub fn is_key_frame(&self) -> bool {
        self.decoded.is_key_frame()
    }
}
