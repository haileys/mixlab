use std::cmp;
use std::fs::File;

use bytes::Buf;
use derive_more::From;
use fdk_aac::enc as aac;
use mpeg2ts::es::StreamId;
use mpeg2ts::time::Timestamp;
use mpeg2ts::ts::{self, TsPacketWriter, TsPacket, TsHeader, TsPayload, ProgramAssociation, ContinuityCounter, Pid, VersionNumber, WriteTsPacket};

use crate::codec::avc::{AvcFrame, AvcError, Millis};
use crate::engine::{InputRef, OutputRef, SAMPLE_RATE};
use crate::module::ModuleT;

use mixlab_protocol::{LineType, Terminal};

#[derive(Debug)]
pub struct TsDump {
    write_ctx: TsWriteCtx,
    time: Millis,
    aac: aac::Encoder,
    aac_output_buff: Vec<u8>,
    audio_pcm_buff: Vec<i16>,
    inputs: Vec<Terminal>,
}

#[derive(Debug)]
struct TsWriteCtx {
    ts: TsPacketWriter<File>,
    audio_continuity_counter: ContinuityCounter,
    video_continuity_counter: ContinuityCounter,
}

impl ModuleT for TsDump {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        let aac_params = aac::EncoderParams {
            bit_rate: aac::BitRate::VbrVeryHigh,
            sample_rate: 44100,
            transport: aac::Transport::Adts,
        };

        let file = File::create("dump.ts").unwrap();
        let mut ts = TsPacketWriter::new(file);

        // write program association table:
        ts.write_ts_packet(&program_association_table()).unwrap();
        ts.write_ts_packet(&program_map_table()).unwrap();

        let write_ctx = TsWriteCtx {
            ts,
            audio_continuity_counter: ContinuityCounter::default(),
            video_continuity_counter: ContinuityCounter::default(),
        };

        let module = TsDump {
            write_ctx,
            time: Millis(0),
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

    fn run_tick(&mut self, _t: u64, inputs: &[InputRef], _: &mut [OutputRef]) -> Option<Self::Indication> {
        let (video, audio) = match inputs {
            [video, audio] => (video.expect_avc(), audio.expect_stereo()),
            _ => unreachable!()
        };

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
        let timestamp = self.time;

        while self.audio_pcm_buff.len() > CHUNK_SAMPLES {
            let chunk_pcm = &self.audio_pcm_buff[0..CHUNK_SAMPLES];
            let mut chunk_offset = 0;

            while chunk_offset < chunk_pcm.len() {
                let encode_result = self.aac.encode(&chunk_pcm[chunk_offset..], &mut aac_buff).expect("aac.encode");
                chunk_offset += encode_result.input_consumed;
                self.aac_output_buff.extend(&aac_buff[0..encode_result.output_size]);
            }

            self.audio_pcm_buff.drain(0..CHUNK_SAMPLES);
            self.time.0 += 10; // 10 ms @ 100 hz, see CHUNK_SAMPLES
        }

        if self.audio_pcm_buff.len() > 0 {
            write_audio(&mut self.write_ctx, &self.aac_output_buff, timestamp).unwrap();
            self.aac_output_buff.truncate(0);
        }

        // process incoming video

        if let Some(frame) = video {
            write_video(&mut self.write_ctx, frame).unwrap();
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

fn write_audio(ctx: &mut TsWriteCtx, mut data: &[u8], timestamp: Millis) -> Result<(), mpeg2ts::Error> {
    use mpeg2ts::pes::PesHeader;
    use mpeg2ts::ts::payload::Bytes;

    let first_payload_len = cmp::min(153, data.len());
    let (first_payload, rest) = data.split_at(first_payload_len);
    data = rest;

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
                pts: Some(Timestamp::new(timestamp.0 * 90)?), // pts is in 90hz or something?
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

fn write_video(ctx: &mut TsWriteCtx, video: &AvcFrame) -> Result<(), WriteVideoError> {
    use mpeg2ts::es::StreamId;
    use mpeg2ts::pes::PesHeader;
    use mpeg2ts::time::{ClockReference, Timestamp};
    use mpeg2ts::ts::AdaptationField;
    use mpeg2ts::ts::payload::{Bytes, Pes};

    let mut buf = video.bitstream.into_bytes()?;
    let pes_data_len = cmp::min(153, buf.remaining());
    let pes_data = buf.split_to(pes_data_len);
    let pcr = ClockReference::new(video.timestamp.0 * 90)?;

    let adaptation_field = if video.frame_type.is_key_frame() {
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

    let pts = Timestamp::new(video.presentation_timestamp.0 * 90)?;
    let dts = Timestamp::new(video.timestamp.0 * 90)?;

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
