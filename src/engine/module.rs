use std::fmt;
use std::future::Future;

use tokio::runtime;
use tokio::sync::mpsc;

use mixlab_protocol::{ModuleParams, Indication, Terminal};

use crate::engine::{InputRef, OutputRef};
use crate::module::{self, ModuleT};
use crate::project::ProjectBaseRef;

#[derive(Debug)]
pub struct ModuleCtx<M: ModuleT> {
    runtime: runtime::Handle,
    base: ProjectBaseRef,
    link: ModuleLink<M>,
}

impl<M: ModuleT> ModuleCtx<M> {
    pub fn project(&self) -> ProjectBaseRef {
        self.base.clone()
    }

    pub fn link(&self) -> ModuleLink<M> {
        self.link.clone()
    }

    pub fn spawn_async(&self, f: impl Future<Output = M::Event> + Send + 'static) {
        let mut link = self.link();
        self.runtime.spawn(async move {
            if let Some(ev) = f.await.into() {
                let _ = link.send_event(ev).await;
            }
        });
    }
}

pub struct ModuleLink<M: ModuleT> {
    events: mpsc::Sender<M::Event>,
}

impl<M: ModuleT> ModuleLink<M> {
    pub async fn send_event(&mut self, ev: M::Event) -> Result<(), ()> {
        self.events.send(ev).await.map_err(|_| ())
    }
}

// must impl Clone manually - derive macro fails because M does not impl Clone
impl<M: ModuleT> Clone for ModuleLink<M> {
    fn clone(&self) -> Self {
        ModuleLink { events: self.events.clone() }
    }
}

impl<M: ModuleT> fmt::Debug for ModuleLink<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ModuleLink")
    }
}

pub struct ModuleHost<M: ModuleT> {
    module: M,
    events: mpsc::Receiver<M::Event>,
}

impl<M: ModuleT> ModuleHost<M> {
    fn new(params: M::Params, base: ProjectBaseRef) -> (Self, M::Indication) {
        let (events_tx, events_rx) = mpsc::channel(2);

        let ctx = ModuleCtx {
            runtime: runtime::Handle::current(),
            base,
            link: ModuleLink { events: events_tx },
        };

        let (module, indication) = M::create(params, ctx);

        let host = ModuleHost {
            module,
            events: events_rx,
        };

        (host, indication)
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
                    if let Some(ev) = self.events.try_recv().ok() {
                        self.module.receive_event(ev);
                    }

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
                        let (host, indication) = ModuleHost::<module::$mod_name::$module>::new(params, base);
                        (Box::new(host) as DynModuleHost, Indication::$module(indication))
                    }
                )*
            }
        }
    }
}

crate::enumerate_modules!{then gen_dyn_module_impls!}
crate::enumerate_modules!{then gen_host_fn!}

pub type DynModuleHost = Box<dyn DynModuleHostT>;
