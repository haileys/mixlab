use std::sync::Arc;

use mixlab_protocol::LineType;
use mixlab_util::time::MediaDuration;

use crate::engine::{CHANNELS, SAMPLES_PER_TICK};
use crate::engine::Sample;
use crate::video;

pub static ZERO_BUFFER_STEREO: [Sample; SAMPLES_PER_TICK * CHANNELS] = [0.0; SAMPLES_PER_TICK * CHANNELS];
pub static ZERO_BUFFER_MONO: [Sample; SAMPLES_PER_TICK] = [0.0; SAMPLES_PER_TICK];

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub data: Arc<video::Frame>,

    // frame timestamp in fractional seconds after enclosing tick begins:
    pub tick_offset: MediaDuration,
}

pub enum InputRef<'a> {
    Disconnected,
    Mono(&'a [Sample]),
    Stereo(&'a [Sample]),
    Video(Option<&'a VideoFrame>),
}

impl<'a> InputRef<'a> {
    pub fn connected(&self) -> bool {
        match self {
            InputRef::Disconnected => false,
            InputRef::Mono(_) |
            InputRef::Stereo(_) |
            InputRef::Video(_) => true,
        }
    }

    pub fn expect_mono(&self) -> &'a [Sample] {
        match self {
            InputRef::Disconnected => &ZERO_BUFFER_MONO,
            InputRef::Mono(buff) => buff,
            InputRef::Stereo(_) => panic!("expected mono input, got stereo"),
            InputRef::Video(_) => panic!("expected mono input, got avc"),
        }
    }

    pub fn expect_stereo(&self) -> &'a [Sample] {
        match self {
            InputRef::Disconnected => &ZERO_BUFFER_STEREO,
            InputRef::Stereo(buff) => buff,
            InputRef::Mono(_) => panic!("expected stereo input, got mono"),
            InputRef::Video(_) => panic!("expected stereo input, got avc"),
        }
    }

    pub fn expect_video(&self) -> Option<&VideoFrame> {
        match self {
            InputRef::Disconnected => None,
            InputRef::Stereo(_) => panic!("expected stereo input, got stereo"),
            InputRef::Mono(_) => panic!("expected stereo input, got mono"),
            InputRef::Video(frame) => *frame,
        }
    }
}

pub enum Output {
    Mono(Vec<Sample>),
    Stereo(Vec<Sample>),
    Video(Option<VideoFrame>),
}

impl Output {
    pub fn from_line_type(line_type: LineType) -> Output {
        match line_type {
            LineType::Mono => Output::Mono(vec![0.0; SAMPLES_PER_TICK]),
            LineType::Stereo => Output::Stereo(vec![0.0; SAMPLES_PER_TICK * CHANNELS]),
            LineType::Video => Output::Video(None),
        }
    }

    pub fn as_input_ref(&self) -> InputRef<'_> {
        match self {
            Output::Mono(buff) => InputRef::Mono(buff),
            Output::Stereo(buff) => InputRef::Stereo(buff),
            Output::Video(packet) => InputRef::Video(packet.as_ref()),
        }
    }

    pub fn as_output_ref(&mut self) -> OutputRef<'_> {
        match self {
            Output::Mono(buff) => OutputRef::Mono(buff),
            Output::Stereo(buff) => OutputRef::Stereo(buff),
            Output::Video(frame) => OutputRef::Video(frame),
        }
    }
}

pub enum OutputRef<'a> {
    Mono(&'a mut [Sample]),
    Stereo(&'a mut [Sample]),
    Video(&'a mut Option<VideoFrame>)
}

impl<'a> OutputRef<'a> {
    pub fn expect_mono(&mut self) -> &mut [Sample] {
        match self {
            OutputRef::Mono(buff) => buff,
            OutputRef::Stereo(_) => panic!("expected mono output, got stereo"),
            OutputRef::Video(_) => panic!("expected mono output, got video"),
        }
    }

    pub fn expect_stereo(&mut self) -> &mut [Sample] {
        match self {
            OutputRef::Stereo(buff) => buff,
            OutputRef::Mono(_) => panic!("expected stereo output, got mono"),
            OutputRef::Video(_) => panic!("expected mono output, got video"),
        }
    }

    pub fn expect_video(&mut self) -> &mut Option<VideoFrame> {
        match self {
            OutputRef::Stereo(_) => panic!("expected stereo output, got video"),
            OutputRef::Mono(_) => panic!("expected mono input, got video"),
            OutputRef::Video(frame) => *frame,
        }
    }
}
