use std::f32;
use std::fmt::{self, Debug};

use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use ringbuf::{RingBuffer, Producer};

use mixlab_protocol::{OutputDeviceParams, OutputDeviceIndication};

use crate::engine::{Sample, CHANNELS};
use crate::module::Module;

pub struct OutputDevice {
    params: OutputDeviceParams,
    host: cpal::Host,
    scratch: Vec<Sample>,
    stream: Option<OutputStream>,
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

        let device = OutputDevice {
            params,
            host,
            scratch: Vec::new(),
            stream: None,
        };

        // TODO - see if we can update devices as they are added/removed from host
        let devices = Some(device.host
            .output_devices()
            .map(|devices| devices
                .flat_map(|device| -> Option<_> {
                    let name = device.name().ok()?;
                    let config = device.default_output_config().ok()?;
                    Some((name, config.channels() as usize))
                })
                .collect())
            .unwrap_or(Vec::new()));

        (device, OutputDeviceIndication { devices })
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

                let stream = output_device.build_output_stream(
                        &config.config(),
                        move |data: &mut [f32]| {
                            let bytes = rx.pop_slice(data);

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
        let input = match inputs[0] {
            Some(input) => input,
            None => return None,
        };

        if let Some(stream) = &mut self.stream {
            let output_channels = stream.config.channels as usize;
            let samples_per_channel = input.len() / CHANNELS;
            let scratch_len = samples_per_channel * output_channels;

            if self.scratch.len() < scratch_len {
                self.scratch.resize(scratch_len, 0.0);
            }

            for i in 0..samples_per_channel {
                if let Some(left) = self.params.left {
                    self.scratch[output_channels * i + left] = input[CHANNELS * i + 0];
                }

                if let Some(right) = self.params.right {
                    self.scratch[output_channels * i + right] = input[CHANNELS * i + 1];
                }
            }

            stream.tx.push_slice(&self.scratch[0..(samples_per_channel * output_channels)]);
        }

        None
    }

    fn input_count(&self) -> usize {
        1
    }

    fn output_count(&self) -> usize {
        0
    }
}
