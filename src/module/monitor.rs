use std::cmp;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Write};
use std::mem;
use std::sync::{Arc, Mutex};

use bytes::{Buf, Bytes, BytesMut, BufMut};
use bytes::buf::BufMutExt;
use derive_more::From;
use fdk_aac::enc as aac;
use futures::sink::SinkExt;
use mpeg2ts::es::StreamId;
use mpeg2ts::time::Timestamp as MpegTimestamp;
use mpeg2ts::ts::{self, TsPacketWriter, TsPacket, TsHeader, TsPayload, ContinuityCounter, Pid, WriteTsPacket};
use mse_fmp4::aac::{AacProfile, SamplingFrequency, ChannelConfiguration};
use mse_fmp4::fmp4::{
    AacSampleEntry, AvcConfigurationBox, AvcSampleEntry, InitializationSegment, MediaDataBox,
    MediaSegment, Mp4Box, Mpeg4EsDescriptorBox, Sample, SampleEntry, SampleFlags, TrackBox,
    TrackExtendsBox, TrackFragmentBox, MovieFragmentHeaderBox, MovieFragmentBox,
};
use mse_fmp4::io::WriteTo;
use num_rational::Rational64;
use tokio::sync::{broadcast, watch, mpsc};
use uuid::Uuid;
use warp::ws::{self, Ws, WebSocket};

use crate::codec::avc::AvcError;
use crate::engine::{InputRef, OutputRef, VideoFrame, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::util::{self, Sequence};

use mixlab_protocol::{LineType, Terminal, MonitorIndication};

lazy_static::lazy_static! {
    static ref SOCKETS: Mutex<HashMap<Uuid, Stream>> = Mutex::new(HashMap::new());
}

struct Stream {
    init: watch::Receiver<Option<Bytes>>,
    segments: Arc<broadcast::Sender<Bytes>>,
}

pub enum ClientKind {
    Ws(WebSocket),
    Http(mpsc::Sender<Bytes>),
}

impl ClientKind {
    pub async fn send(&mut self, data: Bytes) {
        match self {
            ClientKind::Ws(sock) => {
                // TODO it would be great if we didn't need the copy here
                let data = data.to_vec();
                sock.send(ws::Message::binary(data)).await;
            }
            ClientKind::Http(tx) => {
                tx.send(data).await;
            }
        }
    }
}

pub async fn stream(socket_id: Uuid, mut client: ClientKind) {
    let rx = (*SOCKETS).lock()
        .unwrap()
        .get(&socket_id)
        .map(|stream| (stream.init.clone(), stream.segments.subscribe()));

    if let Some((mut init, mut stream)) = rx {
        // get initialisation segment, possibly waiting for it
        loop {
            match init.recv().await {
                None => {
                    // sender half dropped
                    return;
                }
                Some(None) => {
                    // initialisation segment not yet ready
                    continue;
                }
                Some(Some(init)) => {
                    // send initialisation segment to client
                    client.send(init.into()).await;
                    break;
                }
            }
        }

        // TODO if we lag we should catch up to the start of the stream rather
        // than disconnecting the client
        while let Ok(packet) = stream.recv().await {
            client.send(packet).await;
        }
    }
}

const CHANNELS: usize = 2;

// must match AAC encoder's granule size
const SAMPLES_PER_CHANNEL_PER_FRAGMENT: usize = 1024;

#[derive(Debug)]
struct AacFrame {
    data: Vec<u8>,
    timestamp: u64,
}

#[derive(Debug)]
pub struct Monitor {
    socket_id: Uuid,
    init_tx: watch::Sender<Option<Bytes>>,
    segments_tx: Arc<broadcast::Sender<Bytes>>,
    file: File,
    wrote_init: bool,
    write_ctx: WriteCtx,
    engine_epoch: Option<u64>,
    aac: aac::Encoder,
    audio_pcm_buff: Vec<i16>,
    audio_pcm_buff_sample_time: u64,
    inputs: Vec<Terminal>,
}

impl ModuleT for Monitor {
    type Params = ();
    type Indication = MonitorIndication;

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        let (init_tx, init_rx) = watch::channel(None);

        // register socket
        let socket_id = Uuid::new_v4();
        let (segments_tx, _) = broadcast::channel(1024);
        let segments_tx = Arc::new(segments_tx);
        (*SOCKETS).lock().unwrap().insert(socket_id, Stream {
            init: init_rx,
            segments: segments_tx.clone(),
        });

        // setup codecs
        let aac_params = aac::EncoderParams {
            bit_rate: aac::BitRate::VbrVeryHigh,
            sample_rate: 44100,
            transport: aac::Transport::Adts,
        };

        let aac = aac::Encoder::new(aac_params).expect("aac::Encoder::new");

        let file = File::create("dump.mp4").unwrap();

        let write_ctx = WriteCtx {
            sequence: Sequence::new(),
            cumulative_audio_duration: 0,
            cumulative_video_duration: 0,
        };

        let module = Monitor {
            socket_id,
            init_tx,
            segments_tx,
            file,
            wrote_init: false,
            write_ctx,
            engine_epoch: None,
            aac: aac,
            audio_pcm_buff: Vec::new(),
            audio_pcm_buff_sample_time: 0,
            inputs: vec![
                LineType::Video.labeled("Video"),
                LineType::Stereo.labeled("Audio"),
            ]
        };

        (module, MonitorIndication { socket_id })
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, time: u64, inputs: &[InputRef], _: &mut [OutputRef]) -> Option<Self::Indication> {
        let (video, audio) = match inputs {
            [video, audio] => (video.expect_video(), audio.expect_stereo()),
            _ => unreachable!()
        };

        let engine_epoch = *self.engine_epoch.get_or_insert(time);
        let timestamp = Rational64::new((time - engine_epoch) as i64, SAMPLE_RATE as i64);

        // write mp4 init segment
        if !self.wrote_init {
            // init segment requires information from the DCR, so we can't do
            // anything until we've received the first video frame:
            let avc_frame = match video {
                Some(frame) => frame,
                None => { return None; }
            };

            let init = make_mp4_init_segment(&avc_frame);

            println!("writing init");
            let mut init_bytes = BytesMut::new();
            init.write_to((&mut init_bytes).writer()).unwrap();
            let init_bytes = init_bytes.freeze();
            self.file.write_all(&init_bytes).unwrap();
            let _ = self.init_tx.broadcast(Some(init_bytes));

            self.wrote_init = true;
        }

        // convert input audio samples from f32 to i16
        self.audio_pcm_buff.extend(audio.iter().copied().map(|sample| {
            // TODO set CLIP flag if sample is out of range
            let sample = if sample > 1.0 {
                1.0
            } else if sample < -1.0 {
                -1.0
            } else {
                sample
            };

            (sample * i16::max_value() as f32) as i16
        }));

        // process incoming audio
        let audio_frame_sample_count = CHANNELS * SAMPLES_PER_CHANNEL_PER_FRAGMENT;

        let aac_frame =
            if self.audio_pcm_buff.len() > audio_frame_sample_count {
                let fragment_pcm = &self.audio_pcm_buff[0..audio_frame_sample_count];

                let mut aac_buff = [0u8; 4096];

                let encode_result = self.aac.encode(&fragment_pcm, &mut aac_buff).expect("aac.encode");

                if encode_result.input_consumed != audio_frame_sample_count {
                    eprintln!("monitor: aac encoder did not consume exactly {} samples (consumed {})",
                        audio_frame_sample_count, encode_result.input_consumed);
                }

                let frame = AacFrame {
                    data: aac_buff[0..encode_result.output_size].to_vec(),
                    timestamp: self.audio_pcm_buff_sample_time,
                };

                self.audio_pcm_buff_sample_time += SAMPLES_PER_CHANNEL_PER_FRAGMENT as u64;
                self.audio_pcm_buff.drain(0..audio_frame_sample_count);

                Some(frame)
            } else {
                None
            };

        let avc_frame = video;

        let segment = make_mp4_media_segment(
            &mut self.write_ctx,
            avc_frame,
            aac_frame,
            timestamp,
        ).unwrap();

        // only output segment if there is any data to send:
        if segment.mdat_boxes.len() > 0 {
            let mut segment_bytes = BytesMut::new();
            segment.write_to((&mut segment_bytes).writer()).unwrap();
            let segment_bytes = segment_bytes.freeze();
            self.file.write_all(&segment_bytes).unwrap();
            // only fails if no active receives
            let _ = self.segments_tx.send(segment_bytes);
        }

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &[]
    }
}

#[derive(Debug)]
struct WriteCtx {
    sequence: Sequence,
    cumulative_audio_duration: u32,
    cumulative_video_duration: u32,
}

#[derive(From, Debug)]
enum MakeSegmentError {
    MseFmp4(mse_fmp4::Error),
}

const AUDIO_TRACK: u32 = 1;
const VIDEO_TRACK: u32 = 2;

fn make_mp4_init_segment(
    mut video_frame: &VideoFrame,
) -> InitializationSegment {
    use mse_fmp4::avc::AvcDecoderConfigurationRecord;
    use mse_fmp4::fmp4::{
        FileTypeBox, MovieBox, MovieHeaderBox, TrackHeaderBox, MovieExtendsBox,
        MediaBox, MediaHeaderBox, HandlerReferenceBox, MediaInformationBox,
        SoundMediaHeaderBox, DataInformationBox, DataReferenceBox, DataEntryUrlBox,
        SampleTableBox, SampleDescriptionBox, TimeToSampleBox, SampleToChunkBox,
        SampleSizeBox, ChunkOffsetBox, AvcConfigurationBox, VideoMediaHeaderBox,
    };

    let video_dcr = &video_frame.data.specific.bitstream.dcr;

    InitializationSegment {
        ftyp_box: FileTypeBox,
        moov_box: MovieBox {
            mvhd_box: MovieHeaderBox {
                // units of time in this MOV are 1/SAMPLE_RATE seconds:
                timescale: SAMPLE_RATE as u32,
                // no duration outside of extension fragments:
                duration: 0,
            },
            trak_boxes: vec![
                // audio track:
                TrackBox {
                    tkhd_box: TrackHeaderBox {
                        track_id: AUDIO_TRACK,
                        // ISO/IEC 14496-14:2003(E) 5.3:
                        // If the duration of a track cannot be determined,
                        // then the duration is set to all 1s (32-bit maxint)
                        duration: u32::max_value(),
                        volume: 0x0100, // 16.16 fixed point, 0x0100 = 1.0
                        width: video_dcr.width() as u32,
                        height: video_dcr.height() as u32,
                    },
                    edts_box: None,
                    mdia_box: MediaBox {
                        mdhd_box: MediaHeaderBox {
                            timescale: SAMPLE_RATE as u32,
                            duration: 0,
                        },
                        hdlr_box: HandlerReferenceBox {
                            handler_type: *b"soun",
                            name: CString::new("Mixlab Audio").unwrap(),
                        },
                        minf_box: MediaInformationBox {
                            vmhd_box: None,
                            smhd_box: Some(SoundMediaHeaderBox),
                            dinf_box: DataInformationBox {
                                dref_box: DataReferenceBox {
                                    url_box: DataEntryUrlBox,
                                },
                            },
                            stbl_box: SampleTableBox {
                                stsd_box: SampleDescriptionBox {
                                    sample_entries: vec![
                                        SampleEntry::Aac(AacSampleEntry {
                                            esds_box: Mpeg4EsDescriptorBox {
                                                // TODO set these from ADTS header - or are they always constant?
                                                profile: AacProfile::Lc,
                                                frequency: SamplingFrequency::Hz44100,
                                                channel_configuration: ChannelConfiguration::TwoChannels,
                                            },
                                        }),
                                    ],
                                },
                                stts_box: TimeToSampleBox,
                                stsc_box: SampleToChunkBox,
                                stsz_box: SampleSizeBox,
                                stco_box: ChunkOffsetBox,
                            },
                        }
                    },
                },
                // video track:
                TrackBox {
                    tkhd_box: TrackHeaderBox {
                        track_id: VIDEO_TRACK,
                        // ISO/IEC 14496-14:2003(E) 5.3:
                        // If the duration of a track cannot be determined,
                        // then the duration is set to all 1s (32-bit maxint)
                        duration: u32::max_value(),
                        volume: 0x0100, // 16.16 fixed point, 0x0100 = 1.0
                        width: video_dcr.width() as u32,
                        height: video_dcr.height() as u32,
                    },
                    edts_box: None,
                    mdia_box: MediaBox {
                        mdhd_box: MediaHeaderBox {
                            timescale: SAMPLE_RATE as u32,
                            duration: 0,
                        },
                        hdlr_box: HandlerReferenceBox {
                            handler_type: *b"vide",
                            name: CString::new("Mixlab Video").unwrap(),
                        },
                        minf_box: MediaInformationBox {
                            vmhd_box: Some(VideoMediaHeaderBox),
                            smhd_box: None,
                            dinf_box: DataInformationBox {
                                dref_box: DataReferenceBox {
                                    url_box: DataEntryUrlBox,
                                },
                            },
                            stbl_box: SampleTableBox {
                                stsd_box: SampleDescriptionBox {
                                    sample_entries: vec![
                                        SampleEntry::Avc(AvcSampleEntry {
                                            width: 1120, // TOOD set to proper value
                                            height: 720,
                                            avcc_box: AvcConfigurationBox::Raw({
                                                let mut dcr = BytesMut::new();
                                                video_frame.data.specific.bitstream.dcr.write_to(&mut dcr);
                                                println!("DCR: {:?}", dcr);
                                                dcr.freeze().to_vec()
                                            }),
                                        }),
                                    ],
                                },
                                stts_box: TimeToSampleBox,
                                stsc_box: SampleToChunkBox,
                                stsz_box: SampleSizeBox,
                                stco_box: ChunkOffsetBox,
                            },
                        }
                    },
                },
            ],
            mvex_box: MovieExtendsBox {
                mehd_box: None,
                trex_boxes: vec![
                    TrackExtendsBox {
                        track_id: AUDIO_TRACK,
                        default_sample_description_index: 1,
                        default_sample_duration: 0,
                        default_sample_size: 0,
                        default_sample_flags: 0,
                    },
                    TrackExtendsBox {
                        track_id: VIDEO_TRACK,
                        default_sample_description_index: 1,
                        default_sample_duration: 0,
                        default_sample_size: 0,
                        default_sample_flags: 0,
                    },
                ],
            }
        },
    }
}

fn make_mp4_media_segment(
    ctx: &mut WriteCtx,
    mut avc_frame: Option<&VideoFrame>,
    mut aac_frame: Option<AacFrame>,
    tick_timestamp: Rational64,
) -> Result<MediaSegment, MakeSegmentError> {
    use mse_fmp4::fmp4::{
        TrackFragmentHeaderBox, TrackRunBox, TrackFragmentBaseMediaDecodeTimeBox,
    };

    let mut segment = MediaSegment {
        moof_box: MovieFragmentBox {
            mfhd_box: MovieFragmentHeaderBox {
                sequence_number: ctx.sequence.next().get() as u32,
            },
            traf_boxes: Vec::new(),
        },
        mdat_boxes: Vec::new(),
    };

    struct Track {
        traf_idx: usize,
        mdat: MediaDataBox,
    }

    let mut avc_track = None;
    let mut aac_track = None;

    // write AVC track metadata:
    if let Some(avc_frame) = avc_frame {
        // TODO keep a running total of durations/frame timestamps and correct
        // for compounding inaccuracy over time:
        let duration = (avc_frame.data.duration_hint * SAMPLE_RATE as i64).to_integer();

        let mut raw_data = Vec::new();
        avc_frame.data.specific.bitstream.write_to(&mut raw_data);

        let composition_time_offset = (avc_frame.data.specific.composition_time.0 * SAMPLE_RATE as u64) / 1000;

        let sample_flags = SampleFlags {
            is_leading: 0,
            // ISO/IEC 14496-12 8.40.2.3, other samples depend on this:
            sample_depends_on: 1,
            // ISO/IEC 14496-12 8.31.1, false signals a key frame:
            sample_is_non_sync_sample: !avc_frame.data.specific.frame_type.is_key_frame(),
            // should this be 1?
            sample_is_depdended_on: 0,
            sample_has_redundancy: 0,
            sample_padding_value: 0,
            sample_degradation_priority: 0,
        };

        let video_frag = TrackFragmentBox {
            tfhd_box: TrackFragmentHeaderBox {
                track_id: VIDEO_TRACK,
                duration_is_empty: false,
                default_base_is_moof: true,
                base_data_offset: None,
                sample_description_index: None,
                default_sample_duration: None,
                default_sample_size: None,
                default_sample_flags: None,
            },
            tfdt_box: Some(TrackFragmentBaseMediaDecodeTimeBox {
                base_media_decode_time: ctx.cumulative_video_duration,
            }),
            trun_box: TrackRunBox {
                data_offset: Some(0), // dummy for length calculation
                first_sample_flags: None,
                samples: vec![Sample {
                    duration: Some(duration as u32),
                    size: Some(raw_data.len() as u32),
                    composition_time_offset: Some(composition_time_offset as i32),
                    flags: Some(sample_flags),
                }],
            }
        };

        ctx.cumulative_video_duration += duration as u32;

        let mdat = MediaDataBox {
            data: raw_data.to_vec(),
        };

        let traf_idx = segment.moof_box.traf_boxes.len();

        segment.moof_box.traf_boxes.push(video_frag);

        avc_track = Some(Track {
            traf_idx,
            mdat,
        });
    }

    // write AAC track metadata:
    if let Some(mut aac_frame) = aac_frame {
        aac_frame.data.drain(0..7); // snip off 7 byte ADTS header

        let audio_frag = TrackFragmentBox {
            tfhd_box: TrackFragmentHeaderBox {
                track_id: AUDIO_TRACK,
                duration_is_empty: false,
                default_base_is_moof: true,
                base_data_offset: None,
                sample_description_index: None,
                default_sample_duration: None,
                default_sample_size: None,
                default_sample_flags: None,
            },
            tfdt_box: Some(TrackFragmentBaseMediaDecodeTimeBox {
                base_media_decode_time: ctx.cumulative_audio_duration,
            }),
            trun_box: TrackRunBox {
                data_offset: Some(0), // dummy for length calculation
                first_sample_flags: None,
                samples: vec![Sample {
                    duration: Some(SAMPLES_PER_CHANNEL_PER_FRAGMENT as u32),
                    size: Some(aac_frame.data.len() as u32),
                    composition_time_offset: None,
                    flags: None,
                }],
            }
        };

        ctx.cumulative_audio_duration += SAMPLES_PER_CHANNEL_PER_FRAGMENT as u32;

        let mdat = MediaDataBox {
            data: aac_frame.data,
        };

        let traf_idx = segment.moof_box.traf_boxes.len();

        segment.moof_box.traf_boxes.push(audio_frag);

        aac_track = Some(Track {
            traf_idx,
            mdat,
        });
    }

    // write AVC track data and correct data offset in header
    if let Some(avc_track) = avc_track {
        let moof_size = segment.moof_box.box_size().unwrap();

        let mdat_size = segment.mdat_boxes.iter()
            .fold(0, |sz, mdat| sz + mdat.box_size().unwrap());

        let data_offset = moof_size + mdat_size + 8; // account for header in new mdat box

        segment.moof_box.traf_boxes[avc_track.traf_idx]
            .trun_box.data_offset = Some(data_offset as i32);

        segment.mdat_boxes.push(avc_track.mdat);
    }

    // write AAC track data and correct data offset in header
    if let Some(aac_track) = aac_track {
        let moof_size = segment.moof_box.box_size().unwrap();

        let mdat_size = segment.mdat_boxes.iter()
            .fold(0, |sz, mdat| sz + mdat.box_size().unwrap());

        let data_offset = moof_size + mdat_size + 8; // account for header in new mdat box

        segment.moof_box.traf_boxes[aac_track.traf_idx]
            .trun_box.data_offset = Some(data_offset as i32);

        segment.mdat_boxes.push(aac_track.mdat);
    }

    Ok(segment)
}

//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////

#[cfg(feature="mpeg_ts")]
mod ts {

const VIDEO_ES_PID: u16 = 257;
const AUDIO_ES_PID: u16 = 258;

// see https://en.wikipedia.org/wiki/Packetized_elementary_stream
const PES_VIDEO_STREAM_ID: u8 = 0xe0;
const PES_AUDIO_STREAM_ID: u8 = 0xc0;

fn ts_header(pid: u16, continuity_counter: ContinuityCounter) -> TsHeader {
    use mpeg2ts::ts::TransportScramblingControl;

    TsHeader {
        transport_error_indicator: false,
        transport_priority: false,
        pid: Pid::new(pid).unwrap(),
        transport_scrambling_control: TransportScramblingControl::NotScrambled,
        continuity_counter,
    }
}

fn write_audio(ctx: &mut WriteCtx, mut data: &[u8], timestamp: Rational64) -> Result<(), mpeg2ts::Error> {
    use mpeg2ts::pes::PesHeader;
    use mpeg2ts::ts::payload::Bytes;

    let first_payload_len = cmp::min(153, data.len());
    let (first_payload, rest) = data.split_at(first_payload_len);
    data = rest;

    let millis = (timestamp * 1000).to_integer() as u64;

    ctx.ts.write_ts_packet(&TsPacket {
        header: ts_header(AUDIO_ES_PID, ctx.audio_continuity_counter.clone()),
        adaptation_field: None,
        payload: Some(TsPayload::Pes(ts::payload::Pes {
            header: PesHeader {
                stream_id: StreamId::new(PES_AUDIO_STREAM_ID),
                priority: false,
                data_alignment_indicator: false,
                copyright: false,
                original_or_copy: false,
                pts: Some(MpegTimestamp::new(millis * 90)?), // pts is in 90hz or something?
                dts: None,
                escr: None,
            },
            pes_packet_len: 0,
            data: Bytes::new(first_payload)?,
        })),
    })?;

    ctx.audio_continuity_counter.increment();

    while data.len() > 0 {
        let payload_len = cmp::min(Bytes::MAX_SIZE, data.len());
        let (payload, rest) = data.split_at(payload_len);
        data = rest;

        ctx.ts.write_ts_packet(&TsPacket {
            header: ts_header(AUDIO_ES_PID, ctx.audio_continuity_counter.clone()),
            adaptation_field: None,
            payload: Some(TsPayload::Raw(Bytes::new(&payload)?)),
        })?;

        ctx.audio_continuity_counter.increment();
    }

    Ok(())
}

#[derive(From, Debug)]
enum WriteVideoError {
    Mpeg2Ts(mpeg2ts::Error),
    Avc(AvcError),
}

fn write_video(ctx: &mut WriteCtx, frame: &AvcFrame, timestamp: Rational64) -> Result<(), WriteVideoError> {
    use mpeg2ts::es::StreamId;
    use mpeg2ts::pes::PesHeader;
    use mpeg2ts::time::ClockReference;
    use mpeg2ts::ts::AdaptationField;
    use mpeg2ts::ts::payload::{Bytes, Pes};

    let frame_data_timestamp = timestamp + frame.tick_offset;
    let frame_presentation_timestamp = frame_data_timestamp + Rational64::new(frame.data.composition_time.0 as i64, 1000);

    let frame_data_millis = (frame_data_timestamp * 1000).to_integer() as u64;
    let frame_presentation_millis = (frame_presentation_timestamp * 1000).to_integer() as u64;

    let mut buf = frame.data.bitstream.into_bytes()?;
    let pes_data_len = cmp::min(153, buf.remaining());
    let pes_data = buf.split_to(pes_data_len);
    let pcr = ClockReference::new(frame_data_millis * 90)?;

    let adaptation_field = if frame.data.frame_type.is_key_frame() {
        Some(AdaptationField {
            discontinuity_indicator: false,
            random_access_indicator: true,
            es_priority_indicator: false,
            pcr: Some(pcr),
            opcr: None,
            splice_countdown: None,
            transport_private_data: Vec::new(),
            extension: None,
        })
    } else {
        None
    };

    let pts = MpegTimestamp::new(frame_presentation_millis * 90)?;
    let dts = MpegTimestamp::new(frame_data_millis * 90)?;

    let packet = TsPacket {
        header: ts_header(VIDEO_ES_PID, ctx.video_continuity_counter.clone()),
        adaptation_field,
        payload: Some(TsPayload::Pes(Pes {
            header: PesHeader {
                stream_id: StreamId::new(PES_VIDEO_STREAM_ID),
                priority: false,
                data_alignment_indicator: false,
                copyright: false,
                original_or_copy: false,
                pts: Some(pts),
                dts: Some(dts),
                escr: None,
            },
            pes_packet_len: 0,
            data: Bytes::new(&pes_data)?,
        })),
    };

    ctx.ts.write_ts_packet(&packet)?;
    ctx.video_continuity_counter.increment();

    while buf.remaining() > 0 {
        let pes_data_len = cmp::min(Bytes::MAX_SIZE, buf.remaining());
        let pes_data = buf.split_to(pes_data_len);

        let packet = TsPacket {
            header: ts_header(VIDEO_ES_PID, ctx.video_continuity_counter.clone()),
            adaptation_field: None,
            payload: Some(TsPayload::Raw(Bytes::new(&pes_data)?)),
        };

        ctx.ts.write_ts_packet(&packet)?;
        ctx.video_continuity_counter.increment();
    }

    Ok(())
}

}
