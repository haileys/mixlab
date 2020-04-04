use std::f32;
use std::fmt::{self, Debug};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, Duration};

use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use ringbuf::{RingBuffer, Producer};

use mixlab_protocol::{OutputDeviceParams, OutputDeviceIndication, LineType, OutputDeviceWarning};

use crate::engine::{Sample, CHANNELS};
use crate::module::Module;

pub struct OutputDevice {
    params: OutputDeviceParams,
    host: cpal::Host,
    scratch: Vec<Sample>,
    stream: Option<OutputStream>,
    last_clip: Option<Instant>,
    last_lag: Option<Instant>,
    lag_flag: Arc<AtomicBool>,
    indication: OutputDeviceIndication,
}

struct OutputStream {
    tx: Producer<f32>,
    config: cpal::StreamConfig,
    // this field is never used directly but must not be dropped for the
    // stream to continue playing:
    _stream: cpal::Stream,
}

impl Debug for OutputDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OutputDevice {{ params: {:?}, .. }}", self.params)
    }
}

impl Module for OutputDevice {
    type Params = OutputDeviceParams;
    type Indication = OutputDeviceIndication;

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let host = cpal::default_host();

        // TODO - see if we can update devices as they are added/removed from host
        let devices = Some(host.output_devices()
            .map(|devices| devices
                .flat_map(|device| -> Option<_> {
                    let name = device.name().ok()?;
                    let config = device.default_output_config().ok()?;
                    Some((name, config.channels() as usize))
                })
                .collect())
            .unwrap_or(Vec::new()));

        let default_device = host.default_output_device()
            .and_then(|dev| dev.name().ok());

        let indication = OutputDeviceIndication {
            default_device,
            devices,
            clip: None,
            lag: None,
        };

        let device = OutputDevice {
            params,
            host,
            scratch: Vec::new(),
            stream: None,
            last_clip: None,
            last_lag: None,
            lag_flag: Arc::new(AtomicBool::new(false)),
            indication: indication.clone(),
        };

        (device, indication)
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        let OutputDeviceParams { device, left, right } = new_params;

        if self.params.device != device {
            let output_device = self.host.output_devices()
                .ok()
                .and_then(|devices| {
                    devices.into_iter().find(|dev| dev.name().map(|dev| Some(dev) == device).unwrap_or(false))
                });

            if let Some(output_device) = output_device {
                let config = output_device.default_output_config()
                    .expect("default_output_format");

                let (tx, mut rx) = RingBuffer::<f32>::new(65536).split();

                let lag_flag = self.lag_flag.clone();

                let stream = output_device.build_output_stream(
                        &config.config(),
                        move |data: &mut [f32]| {
                            let bytes = rx.pop_slice(data);

                            if bytes < data.len() {
                                lag_flag.store(true, Ordering::Relaxed);
                            }

                            // zero-fill rest of buffer
                            for i in bytes..data.len() {
                                data[i] = 0.0;
                            }
                        },
                        |err| {
                            eprintln!("output stream error! {:?}", err);
                        })
                    .expect("build_output_stream");

                stream.play().expect("stream.play");

                let stream = OutputStream {
                    tx,
                    config: config.config(),
                    _stream: stream,
                };

                self.params.device = device.clone();
                self.stream = Some(stream);
            } else {
                self.stream = None;
            }
        }

        if let Some(stream) = self.stream.as_ref() {
            // zero scratch buffer if channel assignments change so that we don't
            // keep playing left over data:

            if self.params.left != left || self.params.right != right {
                for sample in self.scratch.iter_mut() {
                    *sample = 0.0;
                }
            }

            // assign left and right channels, validating that they are within range:

            self.params.left = left.filter(|left|
                *left < stream.config.channels as usize);

            self.params.right = right.filter(|right|
                *right < stream.config.channels as usize);
        }

        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], _outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let input = inputs[0].unwrap_or(&ZERO_BUFFER_STEREO);

        let mut clip = false;

        if let Some(stream) = &mut self.stream {
            let output_channels = stream.config.channels as usize;
            let samples_per_channel = input.len() / CHANNELS;
            let scratch_len = samples_per_channel * output_channels;

            if self.scratch.len() < scratch_len {
                self.scratch.resize(scratch_len, 0.0);
            }

            for i in 0..samples_per_channel {
                if let Some(left) = self.params.left {
                    let sample = input[CHANNELS * i + 0];

                    if sample < -1.0 || sample > 1.0 {
                        clip = true;
                    }

                    self.scratch[output_channels * i + left] = sample;
                }

                if let Some(right) = self.params.right {
                    let sample = input[CHANNELS * i + 1];

                    if sample < -1.0 || sample > 1.0 {
                        clip = true;
                    }

                    self.scratch[output_channels * i + right] = sample;
                }
            }

            stream.tx.push_slice(&self.scratch[0..(samples_per_channel * output_channels)]);
        }

        let now = Instant::now();

        if clip {
            self.last_clip = Some(now);
        }

        if self.lag_flag.swap(false, Ordering::Relaxed) {
            self.last_lag = Some(now);
        }

        let mut indication_changed = false;

        let new_clip_status = warning_status(
            self.last_clip.map(|time| now - time));

        if self.indication.clip != new_clip_status {
            self.indication.clip = new_clip_status;
            indication_changed = true;
        }

        let new_lag_status = warning_status(
            self.last_lag.map(|time| now - time));

        if self.indication.lag != new_lag_status {
            self.indication.lag = new_lag_status;
            indication_changed = true;
        }

        if indication_changed {
            Some(self.indication.clone())
        } else {
            None
        }
    }

    fn inputs(&self) -> &[LineType] {
        &[LineType::Stereo]
    }

    fn outputs(&self) -> &[LineType] {
        &[]
    }
}

fn warning_status(time_since: Option<Duration>) -> Option<OutputDeviceWarning> {
    const ACTIVE: Duration = Duration::from_millis(100);
    const RECENT: Duration = Duration::from_millis(5000);

    time_since.and_then(|dur| {
        if dur < ACTIVE {
            Some(OutputDeviceWarning::Active)
        } else if dur < RECENT {
            Some(OutputDeviceWarning::Recent)
        } else {
            None
        }
    })
}
