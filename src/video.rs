use std::ptr;
use std::sync::Arc;

use ffmpeg_dev::sys as ff;
use num_rational::Rational64;
use mixlab_codec::avc::AvcFrame;
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

    pub fn id(self: &Arc<Frame>) -> FrameId {
        FrameId(self.clone())
    }
}

pub struct FrameId(Arc<Frame>);

impl PartialEq for FrameId {
    fn eq(&self, other: &Self) -> bool {
        let self_id = &*self.0 as *const Frame;
        let other_id = &*other.0 as *const Frame;

        self_id == other_id
    }
}
