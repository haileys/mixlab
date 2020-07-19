use bytes::BytesMut;
use derive_more::From;
use fdk_aac::enc as aac;
use num_rational::Rational64;
use rml_rtmp::time::RtmpTimestamp;
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use mixlab_protocol::{StreamOutputParams, LineType, Terminal, StreamOutputIndication, StreamOutputLiveStatus};

use crate::engine::{InputRef, OutputRef, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::rtmp::packet::{AudioPacket, VideoPacket, VideoFrameType, VideoPacketType};
use crate::rtmp::client::{self, StreamMetadata, PublishInfo, PublishClient};
use crate::video::encode::{EncodeStream, AudioCtx, AudioParams, VideoCtx, VideoParams, StreamSegment, PixelFormat};

const OUTPUT_WIDTH: usize = 1120;
const OUTPUT_HEIGHT: usize = 700;

#[derive(Debug)]
pub struct StreamOutput {
    params: StreamOutputParams,
    connection: Connection,
    inputs: Vec<Terminal>,
    indication: StreamOutputIndication,
}

impl ModuleT for StreamOutput {
    type Params = StreamOutputParams;
    type Indication = StreamOutputIndication;

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let indic = StreamOutputIndication {
            live: StreamOutputLiveStatus::Offline,
            error: false,
        };

        let module = StreamOutput {
            params,
            connection: Connection::Offline,
            inputs: vec![
                LineType::Video.labeled("Video"),
                LineType::Stereo.labeled("Audio"),
            ],
            indication: indic.clone(),
        };

        (module, indic)
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        if new_params.seq <= self.params.seq {
            // out of date update, reject
            return None;
        }

        if self.connection.is_active() {
            if new_params.disconnect_seq == new_params.seq {
                self.connection = Connection::Offline;

                Some(StreamOutputIndication {
                    live: StreamOutputLiveStatus::Offline,
                    error: false,
                })
            } else {
                // cannot change params on a live stream output
                None
            }
        } else {
            self.params = new_params;

            if self.params.connect_seq == self.params.seq {
                // connect with current details
                let (completion_tx, completion_rx) = oneshot::channel();

                // spawn task to connect to RTMP
                tokio::spawn({
                    let params = self.params.clone();
                    async move {
                        let _ = completion_tx.send(connect_rtmp(params.clone()).await);
                    }
                });

                self.connection = Connection::Connecting(completion_rx);

                Some(StreamOutputIndication {
                    live: StreamOutputLiveStatus::Connecting,
                    error: false,
                })
            } else {
                None
            }
        }
    }

    fn run_tick(&mut self, engine_time: u64, inputs: &[InputRef], _: &mut [OutputRef]) -> Option<Self::Indication> {
        let (video, audio) = match inputs {
            [video, audio] => (video.expect_video(), audio.expect_stereo()),
            _ => unreachable!()
        };

        let timestamp = Rational64::new(engine_time as i64, SAMPLE_RATE as i64);

        let live = match &mut self.connection {
            Connection::Offline => {
                return self.indicate();
            }
            Connection::Failed(_) => {
                return self.indicate();
            }
            Connection::Connecting(completion) => {
                use oneshot::error::TryRecvError;

                match completion.try_recv() {
                    Ok(Ok(publish)) => {
                        self.connection = Connection::Live(LiveOutput::start(timestamp, publish));

                        match &mut self.connection {
                            Connection::Live(live) => live,
                            _ => unreachable!(),
                        }
                    }
                    Ok(Err(e)) => {
                        // failed to connect
                        eprintln!("StreamOutput failed to connect: {:?}", e);
                        self.connection = Connection::Failed(e);
                        return self.indicate();
                    }
                    Err(TryRecvError::Empty) => {
                        // not yet ready
                        return self.indicate();
                    }
                    Err(TryRecvError::Closed) => {
                        // failed to connect
                        self.connection = Connection::Offline;
                        return self.indicate();
                    }
                }
            }
            Connection::Live(live) => live,
        };

        live.encode.send_audio(audio);

        if let Some(video_frame) = video {
            let frame = video_frame.data.decoded.clone();
            let frame_timestamp = timestamp - live.epoch + video_frame.tick_offset;

            live.encode.send_video(frame_timestamp, video_frame.data.duration_hint, frame);
        }

        live.encode.barrier(timestamp - live.epoch);

        while let Some(segment) = live.encode.recv_segment() {
            match segment {
                StreamSegment::Audio { duration: _, timestamp, frame } => {
                    let timestamp = RtmpTimestamp::new((timestamp * 1000).to_integer() as u32);
                    live.publish.publish_audio(AudioPacket::AacRawData(frame), timestamp);
                }
                StreamSegment::Video { duration: _, timestamp, frame } => {
                    let timestamp = RtmpTimestamp::new((timestamp * 1000).to_integer() as u32);
                    live.publish.publish_video(VideoPacket {
                        frame_type: if frame.is_key_frame {
                            VideoFrameType::KeyFrame
                        } else {
                            VideoFrameType::InterFrame
                        },
                        packet_type: VideoPacketType::Nalu,
                        composition_time: frame.composition_time,
                        data: frame.data,
                    }, timestamp);
                }
            }
        }

        return self.indicate();
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self) -> &[Terminal] {
        &[]
    }
}

#[derive(Debug, From)]
enum RtmpConnectError {
    Url(url::ParseError),
    UnsupportedScheme,
    MissingHost,
    Io(tokio::io::Error),
    Client(client::Error),
}

async fn connect_rtmp(params: StreamOutputParams) -> Result<PublishClient, RtmpConnectError> {
    let url = url::Url::parse(&params.rtmp_url)?;

    if url.scheme() != "rtmp" {
        return Err(RtmpConnectError::UnsupportedScheme);
    }

    let hostname = url.host_str().ok_or(RtmpConnectError::MissingHost)?;
    let port = url.port().unwrap_or(1935);

    let path = url.path();
    // url docs guarantee path is /-prefixed except for a specific handful of known urls:
    assert!(path.chars().nth(0) == Some('/'));
    let app_name = &path[1..];

    let conn = TcpStream::connect((hostname, port)).await?;
    conn.set_nodelay(true)?;

    let client = client::start(conn)
        .await?
        .publish(PublishInfo {
            app_name: app_name.to_owned(),
            stream_key: params.rtmp_stream_key.to_owned(),
            meta: StreamMetadata {
                video_width: Some(OUTPUT_WIDTH as u32),
                video_height: Some(OUTPUT_HEIGHT as u32),
                video_codec: None,
                video_frame_rate: None,
                video_bitrate_kbps: None,
                audio_codec: None,
                audio_bitrate_kbps: None,
                audio_sample_rate: None,
                audio_channels: Some(2),
                audio_is_stereo: Some(true),
                encoder: None,
            },
        })
        .await?;

    Ok(client)
}

impl StreamOutput {
    fn indicate(&mut self) -> Option<StreamOutputIndication> {
        let new_indication = match &self.connection {
            Connection::Offline => StreamOutputIndication {
                live: StreamOutputLiveStatus::Offline,
                error: false,
            },
            Connection::Failed(_) => StreamOutputIndication {
                live: StreamOutputLiveStatus::Offline,
                error: true,
            },
            Connection::Connecting(_) => StreamOutputIndication {
                live: StreamOutputLiveStatus::Connecting,
                error: false,
            },
            Connection::Live(_) => StreamOutputIndication {
                live: StreamOutputLiveStatus::Live,
                error: false,
            },
        };

        if new_indication == self.indication {
            // don't send duplicate indication
            None
        } else {
            self.indication = new_indication.clone();
            Some(new_indication)
        }
    }
}

#[derive(Debug)]
enum Connection {
    Offline,
    Failed(RtmpConnectError),
    Connecting(oneshot::Receiver<Result<PublishClient, RtmpConnectError>>),
    Live(LiveOutput),
}

impl Connection {
    pub fn is_active(&self) -> bool {
        match self {
            Connection::Offline => false,
            Connection::Failed(_) => false,
            Connection::Connecting(_) => true,
            Connection::Live(_) => true,
        }
    }
}

#[derive(Debug)]
struct LiveOutput {
    epoch: Rational64,
    encode: EncodeStream,
    publish: PublishClient,
}

impl LiveOutput {
    pub fn start(epoch: Rational64, mut publish: PublishClient) -> Self {
        let audio_ctx = AudioCtx::new(AudioParams {
            bit_rate: aac::BitRate::VbrVeryHigh,
            sample_rate: SAMPLE_RATE,
            transport: aac::Transport::Raw,
        });

        // configuration buffer is ASC when raw transport is in use:
        let asc = audio_ctx.configuration_data();
        publish.publish_audio(AudioPacket::AacSequenceHeader(asc), RtmpTimestamp::new(0));

        let video_ctx = VideoCtx::new(VideoParams {
            width: OUTPUT_WIDTH,
            height: OUTPUT_HEIGHT,
            time_base: SAMPLE_RATE,
            pixel_format: PixelFormat::Yuv420p,
        });

        let mut dsc = BytesMut::new();
        video_ctx.decoder_configuration_record().write_to(&mut dsc);
        let dsc = dsc.freeze();

        publish.publish_video(VideoPacket {
            frame_type: VideoFrameType::KeyFrame,
            packet_type: VideoPacketType::SequenceHeader,
            composition_time: 0,
            data: dsc,
        }, RtmpTimestamp::new(0));

        let encode = EncodeStream::new(audio_ctx, video_ctx);

        LiveOutput {
            epoch,
            encode,
            publish,
        }
    }
}
