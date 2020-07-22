pub mod encode;

use mixlab_codec::ffmpeg::AvFrame;
use mixlab_util::time::MediaDuration;

#[derive(Debug)]
pub struct Frame {
    pub decoded: AvFrame,

    // frame duration in fractional seconds, possibly an estimate if frame
    // duration information is not available:
    pub duration_hint: MediaDuration,
}
