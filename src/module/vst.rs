use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ::vst::host::PluginLoader;
use ::vst::plugin::Plugin;

use mixlab_protocol::{Terminal, LineType};

use crate::engine::{InputRef, OutputRef, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::vst::{self, Host, PluginHandle};

// engine runs at 100hz. we should not assume this, but hardcode for now:
const BLOCK_SIZE: usize = SAMPLE_RATE / 100;

#[derive(Debug)]
pub struct Vst {
    plugin: PluginHandle,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Vst {
    type Params = ();
    type Indication = ();

    fn create(_: ()) -> (Self, ()) {
        let vst = load_vst();
        (vst, ())
    }

    fn update(&mut self, _: ()) -> Option<()> {
        *self = load_vst();
        None
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let inputs = inputs.iter()
            .map(|input| input.expect_mono().to_vec())
            .collect::<Vec<_>>();

        let vst_outputs = self.plugin.process(BLOCK_SIZE, inputs);

        for (out, vst_out) in outputs.iter_mut().zip(vst_outputs) {
            out.expect_mono().copy_from_slice(&vst_out);
        }

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self) -> &[Terminal] {
        &self.outputs
    }
}

fn load_vst() -> Vst {
    let plugin_path = PathBuf::from("vst/SPAN Plus.vst/Contents/MacOS/SPAN Plus");

    assert!(plugin_path.exists());

    let loader = PluginLoader::load(&plugin_path, Arc::new(Mutex::new(Host))).unwrap();

    let plugin = vst::global().open_plugin(loader).unwrap();

    plugin.call(|plugin| {
        plugin.init();
        plugin.set_sample_rate(SAMPLE_RATE as f32);
        plugin.set_block_size(BLOCK_SIZE as i64);
        plugin.resume();
    });

    let inputs = (0..plugin.info.inputs).map(|_| LineType::Mono.unlabeled()).collect();
    let outputs = (0..plugin.info.outputs).map(|_| LineType::Mono.unlabeled()).collect();

    Vst {
        plugin,
        inputs,
        outputs,
    }
}
