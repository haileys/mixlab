use std::f32;
use std::fmt::{self, Debug};

use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use ringbuf::{RingBuffer, Producer};

use mixlab_protocol::{OutputDeviceParams, OutputDeviceIndication};

use crate::engine::{Sample, SAMPLES_PER_TICK};
use crate::module::Module;

pub struct OutputDevice {
    params: OutputDeviceParams,
    host: cpal::Host,
    tx: Option<Producer<f32>>,
    // this field is never used directly but must not be dropped for the
    // stream to continue playing:
    stream: Option<cpal::Stream>,
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
            tx: None,
            stream: None,
        };

        // TODO - see if we can update devices as they are added/removed from host
        let devices = Some(device.host
            .output_devices()
            .map(|devices| devices
                .flat_map(|device| device.name().ok())
                .collect())
            .unwrap_or(Vec::new()));

        (device, OutputDeviceIndication { devices })
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        let OutputDeviceParams { device } = new_params;

        if self.params.device != device {
            let output_device = self.host.output_devices()
                .ok()
                .and_then(|devices| {
                    devices.into_iter().find(|dev| dev.name().map(|dev| Some(dev) == device).unwrap_or(false))
                });

            if let Some(output_device) = output_device {
                let config = output_device.default_output_config()
                    .expect("default_output_format");

                let (tx, mut rx) = RingBuffer::<f32>::new(SAMPLES_PER_TICK * 8).split();

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

                self.params.device = device.clone();
                self.tx = Some(tx);
                self.stream = Some(stream);
            }
        }

        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[&[Sample]], _outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        if let Some(tx) = &mut self.tx {
            tx.push_slice(inputs[0]);
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
