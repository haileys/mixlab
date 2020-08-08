use std::any::Any;

use mixlab_protocol::{MediaId, ModuleParams, Indication, Terminal};

use crate::engine::{InputRef, OutputRef};
use crate::module::{self, ModuleT};
use crate::project::media;
use crate::project::stream::ReadStream;
use crate::project::ProjectBaseRef;

#[derive(Debug)]
pub struct ModuleCtx<M: ModuleT> {
    base: ProjectBaseRef,

    // eventually we'll use this type param
    // use phantom now to prevent a giant refactor later
    phantom: std::marker::PhantomData<M>,
}

impl<M: ModuleT> ModuleCtx<M> {
    pub fn new(base: ProjectBaseRef) -> Self {
        ModuleCtx {
            base,
            phantom: std::marker::PhantomData,
        }
    }
}

pub struct ModuleHost<M> {
    module: M,
}

impl<M: ModuleT> ModuleHost<M> {
    pub fn new(module: M) -> Self {
        ModuleHost { module }
    }
}

pub trait DynModuleHostT {
    fn params(&self) -> ModuleParams;
    fn update(&mut self, new_params: ModuleParams) -> Option<Indication>;
    fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Indication>;
    fn inputs(&self) -> &[Terminal];
    fn outputs(&self) -> &[Terminal];
}

macro_rules! gen_dyn_module_impls {
    ($( $mod_name:ident::$module:ident , )*) => {
        $(
            impl DynModuleHostT for ModuleHost<module::$mod_name::$module> {
                fn params(&self) -> ModuleParams {
                    ModuleParams::$module(self.module.params())
                }

                fn update(&mut self, new_params: ModuleParams) -> Option<Indication> {
                    if let ModuleParams::$module(params) = new_params {
                        self.module.update(params).map(Indication::$module)
                    } else {
                        panic!("module params mismatch! module = {:?}, params = {:?}", self.module, new_params);
                    }
                }

                fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Indication> {
                    self.module.run_tick(t, inputs, outputs)
                        .map(Indication::$module)
                }

                fn inputs(&self) -> &[Terminal] {
                    self.module.inputs()
                }

                fn outputs(&self) -> &[Terminal] {
                    self.module.outputs()
                }
            }
        )*
    }
}

macro_rules! gen_host_fn {
    ($( $mod_name:ident::$module:ident , )*) => {
        pub fn host(params: ModuleParams, base: ProjectBaseRef) -> (DynModuleHost, Indication) {
            match params {
                $(
                    ModuleParams::$module(params) => {
                        let ctx = ModuleCtx::new(base);
                        let (module, indication) = module::$mod_name::$module::create(params, ctx);
                        (DynModuleHost::new(ModuleHost::new(module)), Indication::$module(indication))
                    }
                )*
            }
        }
    }
}

crate::enumerate_modules!{then gen_dyn_module_impls!}
crate::enumerate_modules!{then gen_host_fn!}

pub struct DynModuleHost {
    host: Box<dyn DynModuleHostT>,
}

impl DynModuleHost {
    pub fn new<H: DynModuleHostT + 'static>(host: H) -> Self {
        let host = Box::new(host) as Box<dyn DynModuleHostT>;

        DynModuleHost { host }
    }

    pub fn params(&self) -> ModuleParams {
        self.host.params()
    }

    pub fn update(&mut self, new_params: ModuleParams) -> Option<Indication> {
        self.host.update(new_params)
    }

    pub fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Indication> {
        self.host.run_tick(t, inputs, outputs)
    }

    pub fn inputs(&self) -> &[Terminal] {
        self.host.inputs()
    }

    pub fn outputs(&self) -> &[Terminal] {
        self.host.outputs()
    }
}
