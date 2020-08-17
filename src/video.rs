pub mod encode;

use mixlab_codec::ffmpeg::media::Video;
use mixlab_codec::ffmpeg::AvFrame;
use mixlab_util::time::MediaDuration;

#[derive(Debug, Clone)]
pub struct Frame {
    pub decoded: AvFrame<Video>,

    // frame duration in fractional seconds, possibly an estimate if frame
    // duration information is not available:
    pub duration_hint: MediaDuration,
}
