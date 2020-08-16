use crate::engine::{InputRef, OutputRef};
use crate::module::{ModuleT, ModuleCtx};

use mixlab_protocol::{ShaderParams, LineType, Terminal};

#[derive(Debug)]
pub struct Shader {
    outputs: Vec<Terminal>,
}

impl ModuleT for Shader {
    type Params = ShaderParams;
    type Indication = ();
    type Event = ();

    fn create(_: Self::Params, ctx: ModuleCtx<Self>) -> (Self, Self::Indication) {
        (Self {
            outputs: vec![LineType::Video.unlabeled()],
        }, ())
    }

    fn params(&self) -> Self::Params {
        ShaderParams {}
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _: u64, _: &[InputRef], _: &mut [OutputRef]) -> Option<Self::Indication> {
        None
    }

    fn inputs(&self) -> &[Terminal] {
        &[]
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}
