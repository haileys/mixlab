use std::cmp;
use std::fs::File;
use std::io::{self, Write};

use bytes::{Buf, Bytes};
use derive_more::From;
use fdk_aac::enc as aac;
use mpeg2ts::es::StreamId;
use mpeg2ts::time::Timestamp as MpegTimestamp;
use mpeg2ts::ts::{self, TsPacketWriter, TsPacket, TsHeader, TsPayload, ProgramAssociation, ContinuityCounter, Pid, VersionNumber, WriteTsPacket};
use num_rational::Rational64;

use crate::codec::avc::AvcError;
use crate::engine::{InputRef, OutputRef, AvcFrame, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::util;

use mixlab_protocol::{LineType, Terminal};

use tokio::sync::broadcast;
lazy_static::lazy_static! {
    pub static ref TS_BROADCAST: broadcast::Sender<Bytes> = broadcast::channel(1024).0;
}

#[derive(Debug)]
struct Broadcast<W: Write>(W);

impl<W: Write> Write for Broadcast<W> {
    fn write(&mut self, data: &[u8]) -> Result<usize, io::Error> {
        let n = self.0.write(data)?;
        TS_BROADCAST.send(Bytes::copy_from_slice(&data[0..n]));
        Ok(n)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush()
    }
}

#[derive(Debug)]
pub struct Monitor {
    write_ctx: TsWriteCtx,
    engine_epoch: Option<u64>,
    aac: aac::Encoder,
    aac_output_buff: Vec<u8>,
    audio_pcm_buff: Vec<i16>,
    inputs: Vec<Terminal>,
}

#[derive(Debug)]
struct TsWriteCtx {
    ts: TsPacketWriter<Broadcast<File>>,
    audio_continuity_counter: ContinuityCounter,
    video_continuity_counter: ContinuityCounter,
}

impl ModuleT for Monitor {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        let aac_params = aac::EncoderParams {
            bit_rate: aac::BitRate::VbrVeryHigh,
            sample_rate: 44100,
            transport: aac::Transport::Adts,
        };

        let file = File::create("dump.ts").unwrap();
        let mut ts = TsPacketWriter::new(Broadcast(file));

        // write program association table:
        ts.write_ts_packet(&program_association_table()).unwrap();
        ts.write_ts_packet(&program_map_table()).unwrap();

        let write_ctx = TsWriteCtx {
            ts,
            audio_continuity_counter: ContinuityCounter::default(),
            video_continuity_counter: ContinuityCounter::default(),
        };

        let module = Monitor {
            write_ctx,
            engine_epoch: None,
            aac: aac::Encoder::new(aac_params).expect("aac::Encoder::new"),
            aac_output_buff: Vec::new(),
            audio_pcm_buff: Vec::new(),
            inputs: vec![
                LineType::Avc.labeled("Video"),
                LineType::Stereo.labeled("Audio"),
            ]
        };

        (module, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, time: u64, inputs: &[InputRef], _: &mut [OutputRef]) -> Option<Self::Indication> {
        let (video, audio) = match inputs {
            [video, audio] => (video.expect_avc(), audio.expect_stereo()),
            _ => unreachable!()
        };

        let engine_epoch = *self.engine_epoch.get_or_insert(time);
        let timestamp = Rational64::new((time - engine_epoch) as i64, SAMPLE_RATE as i64);

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

        const CHUNK_SAMPLES: usize = 2 * SAMPLE_RATE / 100;
        let mut aac_buff = [0u8; 4096];

        while self.audio_pcm_buff.len() > CHUNK_SAMPLES {
            let chunk_pcm = &self.audio_pcm_buff[0..CHUNK_SAMPLES];
            let mut chunk_offset = 0;

            while chunk_offset < chunk_pcm.len() {
                let encode_result = self.aac.encode(&chunk_pcm[chunk_offset..], &mut aac_buff).expect("aac.encode");
                chunk_offset += encode_result.input_consumed;
                self.aac_output_buff.extend(&aac_buff[0..encode_result.output_size]);
            }

            self.audio_pcm_buff.drain(0..CHUNK_SAMPLES);
        }

        if self.audio_pcm_buff.len() > 0 {
            write_audio(&mut self.write_ctx, &self.aac_output_buff, timestamp).unwrap();
            self.aac_output_buff.truncate(0);
        }

        // process incoming video
        // TODO
        if let Some(frame) = video {
            write_video(&mut self.write_ctx, &frame, timestamp).unwrap();
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

const PMT_PID: u16 = 256;
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

fn program_association_table() -> TsPacket {
    use mpeg2ts::ts::payload::Pat;
    TsPacket {
        header: ts_header(0, ContinuityCounter::default()),
        adaptation_field: None,
        payload: Some(
            TsPayload::Pat(Pat {
                transport_stream_id: 1,
                version_number: VersionNumber::default(),
                table: vec![
                    ProgramAssociation {
                        program_num: 1,
                        program_map_pid: Pid::new(PMT_PID).unwrap(),
                    }
                ]
            })),
    }
}

fn program_map_table() -> TsPacket {
    use mpeg2ts::{
        ts::{VersionNumber, payload::Pmt, EsInfo},
        es::StreamType,
    };

    TsPacket {
        header: ts_header(PMT_PID, ContinuityCounter::default()),
        adaptation_field: None,
        payload: Some(
            TsPayload::Pmt(Pmt {
                program_num: 1,
                pcr_pid: Some(Pid::new(VIDEO_ES_PID).unwrap()),
                version_number: VersionNumber::default(),
                table: vec![
                    EsInfo {
                        stream_type: StreamType::H264,
                        elementary_pid: Pid::new(VIDEO_ES_PID).unwrap(),
                        descriptors: vec![],
                    },
                    EsInfo {
                        stream_type: StreamType::AdtsAac,
                        elementary_pid: Pid::new(AUDIO_ES_PID).unwrap(),
                        descriptors: vec![],
                    }
                ]
            })),
    }
}

fn write_audio(ctx: &mut TsWriteCtx, mut data: &[u8], timestamp: Rational64) -> Result<(), mpeg2ts::Error> {
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

fn write_video(ctx: &mut TsWriteCtx, frame: &AvcFrame, timestamp: Rational64) -> Result<(), WriteVideoError> {
    use mpeg2ts::es::StreamId;
    use mpeg2ts::pes::PesHeader;
    use mpeg2ts::time::ClockReference;
    use mpeg2ts::ts::AdaptationField;
    use mpeg2ts::ts::payload::{Bytes, Pes};

    // println!("[TS-DUMP] timestamp: {}, tick_offset: {}, comp_time: {}",
    //     util::decimal(timestamp), util::decimal(frame.tick_offset), frame.data.composition_time.0);

    let frame_data_timestamp = timestamp + frame.tick_offset;
    let frame_presentation_timestamp = frame_data_timestamp + Rational64::new(frame.data.composition_time.0 as i64, 1000);

    let frame_data_millis = (frame_data_timestamp * 1000).to_integer() as u64;
    let frame_presentation_millis = (frame_presentation_timestamp * 1000).to_integer() as u64;

    // println!("frame_data_millis: {}", frame_data_millis);

    let mut buf = frame.data.bitstream.into_bytes()?;
    let pes_data_len = cmp::min(153, buf.remaining());
    let pes_data = buf.split_to(pes_data_len);
    let pcr = ClockReference::new(frame_data_millis * 90)?;

    let adaptation_field = if frame.data.frame_type.is_key_frame() {
        println!("[TS-DUMP] key frame!");
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
