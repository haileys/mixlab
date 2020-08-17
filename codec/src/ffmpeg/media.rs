// ffmpeg media types

use ffmpeg_dev::sys as ff;

pub trait MediaType {
    const FFMPEG_MEDIA_TYPE: ff::AVMediaType;
}

#[derive(Debug)]
pub struct Video;

impl MediaType for Video {
    const FFMPEG_MEDIA_TYPE: ff::AVMediaType = ff::AVMediaType_AVMEDIA_TYPE_VIDEO;
}
