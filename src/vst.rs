use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::num::NonZeroUsize;
use std::os::raw::c_void;
use std::ptr;
use std::sync::mpsc::{self, SyncSender};
use std::sync::Mutex;

use once_cell::sync::OnceCell;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winit::dpi::LogicalSize;
use winit::event::Event;
use winit::event_loop::{EventLoop, EventLoopProxy, ControlFlow, EventLoopWindowTarget};
use winit::window::{Window, WindowBuilder};
use ::vst::api;
use ::vst::editor::Rect;
use ::vst::host::{Dispatch, PluginLoader, PluginInstance, PluginLoadError};
use ::vst::plugin::OpCode;

use crate::util::Sequence;

static GLOBAL_CONTEXT: OnceCell<VstContext> = OnceCell::new();

type PluginId = NonZeroUsize;

type PluginCallFn = Box<dyn FnMut(&mut PluginInstance) + Send>;

enum Msg {
    OpenPlugin(PluginLoader<Host>, SyncSender<Result<PluginHandle, PluginLoadError>>),
    CallPlugin(PluginId, PluginCallFn),
    ClosePlugin(PluginId),
}

#[derive(Debug)]
pub struct VstContext {
    proxy: Mutex<EventLoopProxy<Msg>>,
}

pub fn global() -> &'static VstContext {
    GLOBAL_CONTEXT.get().expect("vst::GLOBAL_CONTEXT is not initialised")
}

fn set_global(context: VstContext) {
    GLOBAL_CONTEXT.set(context)
        .expect("attempted to reinitialise vst::GLOBAL_CONTEXT");
}

/// winit's event loop must be set up on the main thread on macOS
pub fn hijack_main_thread() -> ! {
    let event_loop = EventLoop::<Msg>::with_user_event();

    let proxy = Mutex::new(event_loop.create_proxy());
    set_global(VstContext { proxy });

    let mut state = State {
        plugin_seq: Sequence::new(),
        plugin_instances: HashMap::new(),
    };

    event_loop.run(move |event, event_loop, cflow| {
        *cflow = ControlFlow::Wait;
        handle_event(&mut state, &event_loop, event);
    });
}

struct State {
    plugin_seq: Sequence,
    plugin_instances: HashMap<PluginId, (PluginInstance, Window)>,
}

fn handle_event(state: &mut State, event_loop: &EventLoopWindowTarget<Msg>, ev: Event<Msg>) {
    match ev {
        Event::NewEvents(_) => {}
        Event::WindowEvent { window_id: _, event: _ } => {
            // println!("WindowEvent({:?}): {:?}", window_id, event);
        }
        Event::DeviceEvent { device_id: _, event: _ } => {
            // println!("DeviceEvent({:?}): {:?}", device_id, event);
        }
        Event::UserEvent(msg) => {
            match msg {
                Msg::OpenPlugin(plugin_loader, retn) => {
                    let result = open_plugin(event_loop, plugin_loader)
                        .map(|plugin_data| {
                            let plugin_id = state.plugin_seq.next();
                            state.plugin_instances.insert(plugin_id, plugin_data);
                            PluginHandle { plugin_id }
                        });

                    let _ = retn.send(result);
                }
                Msg::CallPlugin(plugin_id, mut f) => {
                    f(&mut state.plugin_instances.get_mut(&plugin_id).unwrap().0);
                }
                Msg::ClosePlugin(plugin_id) => {
                    // TOOD close window too
                    state.plugin_instances.remove(&plugin_id);
                }
            }
        }
        Event::Suspended => {}
        Event::Resumed => {}
        Event::MainEventsCleared => {}
        Event::RedrawRequested(_) => {
            // println!("redrawing window: {:?}", window_id);
        }
        Event::RedrawEventsCleared => {}
        Event::LoopDestroyed => {}
    }
}

fn open_plugin(
    event_loop: &EventLoopWindowTarget<Msg>,
    mut plugin_loader: PluginLoader<Host>,
) -> Result<(PluginInstance, Window), PluginLoadError> {
    let instance = plugin_loader.instance()?;

    let (editor_width, editor_height) = unsafe {
        let mut rect = ptr::null::<Rect>();

        instance.dispatch(OpCode::EditorGetRect, 0, 0, &mut rect as *mut *const _ as *mut c_void, 0.0);

        if rect != ptr::null() {
            let rect = *rect;
            (rect.right - rect.left, rect.bottom - rect.top)
        } else {
            panic!("EditorGetRect failed");
        }
    };

    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(editor_width, editor_height))
        .with_resizable(false)
        .with_title("Mixlab VST")
        .build(event_loop)
        .unwrap();

    let handle = window.raw_window_handle();

    let handle_ptr = match handle {
        RawWindowHandle::MacOS(macos) => macos.ns_view,
        _ => panic!("don't know this platform"),
    };

    unsafe {
        instance.dispatch(OpCode::EditorOpen, 0, 0, handle_ptr, 0.0);
    }

    Ok((instance, window))
}

impl VstContext {
    fn send_event(&self, msg: Msg) {
        self.proxy.lock().unwrap().send_event(msg).unwrap()
    }

    pub fn open_plugin(&self, loader: PluginLoader<Host>) -> Result<PluginHandle, PluginLoadError> {
        let (retn_tx, retn) = mpsc::sync_channel(1);
        self.send_event(Msg::OpenPlugin(loader, retn_tx));
        retn.recv().unwrap()
    }
}

#[derive(Debug)]
pub struct PluginHandle {
    plugin_id: PluginId,
}

impl PluginHandle {
    pub fn call<Ret: Send + 'static>(&self, mut f: impl FnMut(&mut PluginInstance) -> Ret + Send + 'static) -> Ret {
        let (retn_tx, retn) = mpsc::sync_channel(1);

        let f = Box::new(move |plugin: &mut PluginInstance| {
            let _ = retn_tx.send(f(plugin));
        }) as PluginCallFn;

        global().send_event(Msg::CallPlugin(self.plugin_id, f));

        retn.recv().unwrap()
    }
}

impl Drop for PluginHandle {
    fn drop(&mut self) {
        global().send_event(Msg::ClosePlugin(self.plugin_id))
    }
}

impl Debug for Msg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Msg::OpenPlugin(..) => write!(f, "Msg::OpenPlugin(..)"),
            Msg::CallPlugin(..) => write!(f, "Msg::CallPlugin(..)"),
            Msg::ClosePlugin(..) => write!(f, "Msg::ClosePlugin(..)"),
        }
    }
}

pub struct Host;

impl ::vst::host::Host for Host {
    fn automate(&self, index: i32, value: f32) {
        eprintln!("automate: index = {:?}; value = {:?}", index, value);
    }

    fn get_plugin_id(&self) -> i32 { todo!() }

    fn idle(&self) { todo!() }

    fn get_info(&self) -> (isize, String, String) {
        (1, "Mixlab".to_owned(), "Mixlab".to_owned())
    }

    fn process_events(&self, _events: &api::Events) { todo!() }

    fn get_time_info(&self, _mask: i32) -> Option<api::TimeInfo> { todo!() }

    fn get_block_size(&self) -> isize { todo!() }

    fn update_display(&self) { todo!() }
}
