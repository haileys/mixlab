use std::collections::{BTreeMap, HashSet};
use std::mem;

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlElement, HtmlCanvasElement, MouseEvent, Element};
use yew::{html, Callback, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

use mixlab_protocol::{ModuleId, TerminalId, InputId, OutputId, ModuleParams, OscillatorParams, Waveform, WorkspaceOp, WindowGeometry, Coords, Indication, OutputDeviceParams, FmSineParams, AmplifierParams, GateState, LineType, EnvelopeParams, MixerParams, StreamInputParams, EqThreeParams, StreamOutputParams, VideoMixerParams, MediaSourceParams, ShaderParams};

use crate::component::midi_target::MidiUiMode;
use crate::module::amplifier::Amplifier;
use crate::module::envelope::Envelope;
use crate::module::eq_three::EqThree;
use crate::module::fm_sine::FmSine;
use crate::module::media_source::MediaSource;
use crate::module::mixer::Mixer;
use crate::module::monitor::Monitor;
use crate::module::oscillator::Oscillator;
use crate::module::output_device::OutputDevice;
use crate::module::plotter::Plotter;
use crate::module::stream_input::StreamInput;
use crate::module::stream_output::StreamOutput;
use crate::module::trigger::Trigger;
use crate::module::video_mixer::VideoMixer;
use crate::util::{self, stop_propagation, prevent_default, Sequence};
use crate::session::{WorkspaceStateRef, WorkspaceState, SessionRef};
use crate::{App, AppMsg};

pub struct Workspace {
    link: ComponentLink<Self>,
    props: WorkspaceProps,
    workspace_ref: NodeRef,
    gen_z_index: Sequence,
    mouse: MouseMode,
    window_refs: BTreeMap<ModuleId, WindowRef>,
}

#[derive(Properties, Clone)]
pub struct WorkspaceProps {
    pub app: ComponentLink<App>,
    pub state: WorkspaceStateRef,
    pub session: SessionRef,
}

pub enum MouseMode {
    Normal,
    Drag(Drag),
    Connect(TerminalId, TerminalRef, Option<Coords>),
    ContextMenu(Coords),
}

pub struct Drag {
    module: ModuleId,
    origin: Coords,
}

#[derive(Debug)]
pub enum WorkspaceMsg {
    Rerender,
    DragStart(ModuleId, MouseEvent),
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    MouseMove(MouseEvent),
    SelectTerminal(TerminalId, TerminalRef),
    ClearTerminal(TerminalId),
    DeleteWindow(ModuleId),
    UpdateModuleParams(ModuleId, ModuleParams),
    CreateModule(ModuleParams, Coords),
}

impl Component for Workspace {
    type Message = WorkspaceMsg;
    type Properties = WorkspaceProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut workspace = Workspace {
            link,
            props,
            workspace_ref: NodeRef::default(),
            gen_z_index: Sequence::new(),
            mouse: MouseMode::Normal,
            window_refs: BTreeMap::new(),
        };

        workspace.update_state();

        workspace
    }

    fn change(&mut self, new_props: Self::Properties) -> ShouldRender {
        self.props = new_props;
        self.update_state();
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        return match msg {
            WorkspaceMsg::Rerender => {
                true
            }
            WorkspaceMsg::DragStart(module, ev) => {
                let mut state = self.props.state.borrow_mut();

                if let Some(geom) = state.geometry.get_mut(&module) {
                    self.mouse = MouseMode::Drag(Drag {
                        module,
                        origin: Coords { x: ev.page_x(), y: ev.page_y() },
                    });

                    geom.z_index = self.gen_z_index.next().get();

                    true
                } else {
                    false
                }
            }
            WorkspaceMsg::MouseDown(ev) => {
                const RIGHT_MOUSE_BUTTON: u16 = 2;

                if (ev.buttons() & RIGHT_MOUSE_BUTTON) != 0 {
                    match self.mouse {
                        MouseMode::Connect(..) => {
                            self.mouse = MouseMode::Normal;
                        }
                        MouseMode::Normal | MouseMode::ContextMenu(_) => {
                            let mouse_loc = Coords { x: ev.offset_x(), y: ev.offset_y() };
                            self.mouse = MouseMode::ContextMenu(mouse_loc);
                        }
                        MouseMode::Drag(_) => {}
                    }

                    true
                } else {
                    match self.mouse {
                        MouseMode::Normal | MouseMode::Drag(_) => {
                            false
                        }
                        MouseMode::Connect(..) | MouseMode::ContextMenu(_) => {
                            self.mouse = MouseMode::Normal;
                            true
                        }
                    }
                }
            }
            WorkspaceMsg::MouseUp(ev) => {
                match self.mouse {
                    MouseMode::Normal => false,
                    MouseMode::Drag(ref mut drag) => {
                        let mut state = self.props.state.borrow_mut();

                        let should_render = drag_event(&mut state, &self.window_refs, drag, ev);

                        if let Some(geometry) = state.geometry.get(&drag.module) {
                            self.props.app.send_message(
                                AppMsg::ClientUpdate(
                                    WorkspaceOp::UpdateWindowGeometry(drag.module, geometry.clone())));
                        }

                        self.mouse = MouseMode::Normal;

                        should_render
                    }
                    MouseMode::Connect(..) => false,
                    MouseMode::ContextMenu(..) => false,
                }
            }
            WorkspaceMsg::MouseMove(ev) => {
                match &mut self.mouse {
                    MouseMode::Normal | MouseMode::ContextMenu(_) => false,
                    MouseMode::Drag(ref mut drag) => {
                        drag_event(&mut self.props.state.borrow_mut(), &self.window_refs, drag, ev)
                    }
                    MouseMode::Connect(_, _, ref mut coords) => {
                        let workspace = self.workspace_ref.cast::<HtmlElement>().unwrap();
                        let target = ev.target().and_then(|target| target.dyn_into::<Element>().ok()).unwrap();
                        let target_offset_coords = util::offset_coords_in(workspace, target).expect("offset_coords_in");
                        *coords = Some(target_offset_coords.add(Coords {
                            x: ev.offset_x(),
                            y: ev.offset_y(),
                        }));
                        true
                    }
                }
            }
            WorkspaceMsg::SelectTerminal(terminal_id, terminal_ref) => {
                match &self.mouse {
                    MouseMode::Normal | MouseMode::ContextMenu(_) => {
                        self.mouse = MouseMode::Connect(terminal_id, terminal_ref, None);
                        false
                    }
                    MouseMode::Connect(other_terminal_id, other_terminal_ref, _) => {
                        match (terminal_id, *other_terminal_id) {
                            (TerminalId::Input(input), TerminalId::Output(output)) |
                            (TerminalId::Output(output), TerminalId::Input(input)) => {
                                let mut state = self.props.state.borrow_mut();

                                if terminal_ref.line_type == other_terminal_ref.line_type {
                                    state.connections.insert(input, output);

                                    self.mouse = MouseMode::Normal;

                                    self.props.app.send_message(
                                        AppMsg::ClientUpdate(
                                            WorkspaceOp::CreateConnection(input, output)));

                                    true
                                } else {
                                    // type mismatch on connection, don't let the user connect it.
                                    // TODO - should we show an error or an icon or something?
                                    false
                                }
                            }
                            _ => {
                                // invalid connection, don't let the user do it
                                false
                            }
                        }
                    }
                    MouseMode::Drag(_) => false,
                }
            }
            WorkspaceMsg::ClearTerminal(terminal) => {
                match terminal {
                    TerminalId::Input(input) => {
                        self.props.state.borrow_mut()
                            .connections
                            .remove(&input);

                        self.props.app.send_message(
                            AppMsg::ClientUpdate(
                                WorkspaceOp::DeleteConnection(input)));
                    }
                    TerminalId::Output(output) => {
                        let mut msgs = Vec::new();

                        let mut state = self.props.state.borrow_mut();

                        for (in_, out_) in &state.connections {
                            if *out_ == output {
                                msgs.push(AppMsg::ClientUpdate(
                                    WorkspaceOp::DeleteConnection(*in_)));
                            }
                        }

                        // yeah, this is just doing the same loop as the loop above
                        // but it's good enough for now
                        state.connections.retain(|_, out| output != *out);

                        self.props.app.send_message_batch(msgs);
                    }
                }
                true
            }
            WorkspaceMsg::DeleteWindow(module) => {
                let mut state = self.props.state.borrow_mut();
                state.modules.remove(&module);
                state.geometry.remove(&module);
                state.connections.retain(|input, output| {
                    output.module_id() != module && input.module_id() != module
                });

                self.props.app.send_message(
                    AppMsg::ClientUpdate(
                        WorkspaceOp::DeleteModule(module)));

                true
            }
            WorkspaceMsg::UpdateModuleParams(module, params) => {
                let mut state = self.props.state.borrow_mut();

                if let Some(module_params) = state.modules.get_mut(&module) {
                    // verify that we're updating the module params with the
                    // same kind of module params:
                    if mem::discriminant(&*module_params) == mem::discriminant(&params) {
                        *module_params = params.clone();

                        self.props.app.send_message(
                            AppMsg::ClientUpdate(
                                WorkspaceOp::UpdateModuleParams(module, params)));

                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            WorkspaceMsg::CreateModule(module, coords) => {
                self.mouse = MouseMode::Normal;

                let geometry = WindowGeometry {
                    position: coords,
                    z_index: self.gen_z_index.next().get(),
                };

                self.props.app.send_message(
                    AppMsg::ClientUpdate(
                        WorkspaceOp::CreateModule(module, geometry)));

                true
            }
        };

        fn drag_event(state: &mut WorkspaceState, window_refs: &BTreeMap<ModuleId, WindowRef>, drag: &mut Drag, ev: MouseEvent) -> ShouldRender {
            let mouse_pos = Coords { x: ev.page_x(), y: ev.page_y() };

            let delta = mouse_pos.sub(drag.origin);
            drag.origin = mouse_pos;

            if let Some(geom) = state.geometry.get_mut(&drag.module) {
                geom.position = geom.position.add(delta);

                let el = window_refs.get(&drag.module)
                    .and_then(|refs| refs.module.cast::<HtmlElement>());

                if let Some(el) = el {
                    let style = el.style();
                    let _ = style.set_property("left", &format!("{}px", geom.position.x));
                    let _ = style.set_property("top", &format!("{}px", geom.position.y));
                }

                true
            } else {
                false
            }
        }
    }

    fn view(&self) -> Html {
        let mut connections: Vec<(Coords, Coords)> = vec![];

        for (input, output) in &self.props.state.borrow().connections {
            if let Some(input_coords) = self.screen_coords_for_terminal(TerminalId::Input(*input)) {
                if let Some(output_coords) = self.screen_coords_for_terminal(TerminalId::Output(*output)) {
                    connections.push((output_coords, input_coords));
                }
            }
        }

        if let MouseMode::Connect(terminal_id, _, Some(to_coords)) = &self.mouse {
            if let Some(start_coords) = self.screen_coords_for_terminal(*terminal_id) {
                let pair = match terminal_id {
                    TerminalId::Input(_) => (*to_coords, start_coords),
                    TerminalId::Output(_) => (start_coords, *to_coords),
                };

                connections.push(pair);
            }
        }

        html! {
            <div class="workspace"
                ref={self.workspace_ref.clone()}
                onmousemove={self.link.callback(WorkspaceMsg::MouseMove)}
                oncontextmenu={prevent_default()}
            >
                <div class="workspace-event-target"
                    onmouseup={self.link.callback(WorkspaceMsg::MouseUp)}
                    onmousedown={self.link.callback(WorkspaceMsg::MouseDown)}
                />

                { for self.window_refs.iter().map(|(id, refs)| {
                    let state = self.props.state.borrow();
                    let module = state.modules.get(id);
                    let geometry = state.geometry.get(id);
                    let workspace = self.link.clone();
                    let indication = state.indications.get(id);

                    if let (Some(module), Some(geometry)) = (module, geometry) {
                        let name = format!("{:?}", module).chars().take_while(|c| c.is_alphanumeric()).collect::<String>();
                        html! { <Window
                            id={id}
                            module={module}
                            refs={refs}
                            name={name}
                            workspace={workspace}
                            geometry={geometry}
                            indication={indication.cloned()}
                            session={self.props.session.clone()}
                        /> }
                    } else {
                        html! {}
                    }
                }) }

                <Connections connections={connections} />

                {self.view_context_menu()}
            </div>
        }
    }

    fn rendered(&mut self, first_render: bool) {
        // need to re-render immediately after first render so that we have
        // screen coordinates from noderefs to pass to Connections
        if first_render {
            self.link.send_message(WorkspaceMsg::Rerender);
        }
    }
}

impl Workspace {
    fn update_state(&mut self) {
        let mut deleted_windows = self.window_refs.keys().copied().collect::<HashSet<_>>();

        let state = self.props.state.borrow();

        for id in state.modules.keys() {
            if deleted_windows.remove(id) {
                // cool, nothing changes with this module
            } else {
                // this module was not present before, create a window ref for it
                let inputs = state.inputs.get(id);
                let outputs = state.outputs.get(id);

                if let (Some(inputs), Some(outputs)) = (inputs, outputs) {
                    let refs = WindowRef {
                        module: NodeRef::default(),
                        inputs: make_terminal_refs(inputs, TerminalType::Input),
                        outputs: make_terminal_refs(outputs, TerminalType::Output),
                    };

                    self.window_refs.insert(*id, refs);

                    fn make_terminal_refs(terminals: &[mixlab_protocol::Terminal], terminal_type: TerminalType) -> Vec<TerminalRef> {
                        terminals.iter()
                            .cloned()
                            .map(|terminal| TerminalRef {
                                node: NodeRef::default(),
                                label: terminal.label().map(String::from),
                                line_type: terminal.line_type(),
                                terminal_type,
                            })
                            .collect()
                    }
                }
            }
        }

        for deleted_window in deleted_windows {
            self.window_refs.remove(&deleted_window);
        }
    }

    fn screen_coords_for_terminal(&self, terminal_id: TerminalId) -> Option<Coords> {
        let state = self.props.state.borrow();
        let geometry = state.geometry.get(&terminal_id.module_id())?;
        let refs = self.window_refs.get(&terminal_id.module_id())?;

        let terminal_ref = match terminal_id {
            TerminalId::Input(InputId(_, index)) => refs.inputs.get(index)?,
            TerminalId::Output(OutputId(_, index)) => refs.outputs.get(index)?,
        };

        let terminal_node = terminal_ref.node.cast::<HtmlElement>()?;

        let terminal_coords = Coords { x: terminal_node.offset_left() + 9, y: terminal_node.offset_top() + 9 };
        Some(geometry.position.add(terminal_coords))
    }

    fn view_context_menu(&self) -> Html {
        let coords = match self.mouse {
            MouseMode::ContextMenu(coords) => coords,
            _ => return html! {},
        };

        let items = &[
            ("Oscillator", ModuleParams::Oscillator(OscillatorParams { freq: 100.0, waveform: Waveform::Sine })),
            ("Mixer (2 channel)", ModuleParams::Mixer(MixerParams::with_channels(2))),
            ("Mixer (4 channel)", ModuleParams::Mixer(MixerParams::with_channels(4))),
            ("Mixer (8 channel)", ModuleParams::Mixer(MixerParams::with_channels(8))),
            ("Output Device", ModuleParams::OutputDevice(OutputDeviceParams { device: None, left: None, right: None })),
            ("Plotter", ModuleParams::Plotter(())),
            ("FM Sine", ModuleParams::FmSine(FmSineParams { freq_lo: 90.0, freq_hi: 110.0 })),
            ("Amplifier", ModuleParams::Amplifier(AmplifierParams { amplitude: 1.0, mod_depth: 0.5 })),
            ("Trigger", ModuleParams::Trigger(GateState::Closed)),
            ("Envelope", ModuleParams::Envelope(EnvelopeParams::default())),
            ("Stereo Panner", ModuleParams::StereoPanner(())),
            ("Stereo Splitter", ModuleParams::StereoSplitter(())),
            ("Stream Input", ModuleParams::StreamInput(StreamInputParams::default())),
            ("Stream Output", ModuleParams::StreamOutput(StreamOutputParams::default())),
            ("EQ Three", ModuleParams::EqThree(EqThreeParams::default())),
            ("Monitor", ModuleParams::Monitor(())),
            ("Video Mixer", ModuleParams::VideoMixer(VideoMixerParams::default())),
            ("Media Source", ModuleParams::MediaSource(MediaSourceParams::default())),
            ("Shader", ModuleParams::Shader(ShaderParams::default())),
        ];

        html! {
            <div class="context-menu"
                style={format!("left:{}px; top:{}px;", coords.x, coords.y)}
                onmousedown={stop_propagation()}
            >
                <div class="context-menu-heading">{"Add module"}</div>
                { for items.iter().map(|(label, params)| {
                    let params = params.clone();

                    html! {
                        <div class="context-menu-item"
                            onmousedown={self.link.callback(move |_|
                                WorkspaceMsg::CreateModule(params.clone(), coords))}
                        >
                            {label}
                        </div>
                    }
                }) }
            </div>
        }
    }
}

pub struct Window {
    link: ComponentLink<Self>,
    props: WindowProps,
    midi_mode: MidiUiMode,
}

pub enum WindowMsg {
    DragStart(MouseEvent),
    TerminalMouseDown(MouseEvent, TerminalId, TerminalRef),
    Delete,
    UpdateParams(ModuleParams),
    SetMidiMode(MidiUiMode),
}

#[derive(Properties, Clone, Debug)]
pub struct WindowProps {
    pub id: ModuleId,
    pub module: ModuleParams,
    pub geometry: WindowGeometry,
    pub name: String,
    pub workspace: ComponentLink<Workspace>,
    pub refs: WindowRef,
    pub indication: Option<Indication>,
    pub session: SessionRef,
}

#[derive(Clone, Debug)]
pub struct WindowRef {
    module: NodeRef,
    inputs: Vec<TerminalRef>,
    outputs: Vec<TerminalRef>,
}

#[derive(Clone, Copy, Debug)]
enum TerminalType {
    Input,
    Output
}

impl TerminalType {
    fn to_css_name(&self) -> &str {
        match self {
            TerminalType::Input => "input",
            TerminalType::Output => "output",
        }
    }
}

#[derive(Clone, Debug)]
pub struct TerminalRef {
    label: Option<String>,
    node: NodeRef,
    line_type: LineType,
    terminal_type: TerminalType,
}

impl Component for Window {
    type Message = WindowMsg;
    type Properties = WindowProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Window {
            link,
            props,
            midi_mode: MidiUiMode::Normal,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            WindowMsg::DragStart(ev) => {
                self.props.workspace.send_message(
                    WorkspaceMsg::DragStart(self.props.id, ev));

                false
            }
            WindowMsg::TerminalMouseDown(ev, terminal_id, terminal_ref) => {
                let msg =
                    if (ev.buttons() & 2) != 0 {
                        // right click
                        WorkspaceMsg::ClearTerminal(terminal_id)
                    } else {
                        WorkspaceMsg::SelectTerminal(terminal_id, terminal_ref)
                    };

                self.props.workspace.send_message(msg);

                false
            }
            WindowMsg::Delete => {
                self.props.workspace.send_message(
                    WorkspaceMsg::DeleteWindow(self.props.id));

                false
            }
            WindowMsg::UpdateParams(params) => {
                self.props.workspace.send_message(
                    WorkspaceMsg::UpdateModuleParams(self.props.id, params));

                false
            }
            WindowMsg::SetMidiMode(new_midi_mode) => {
                self.midi_mode = new_midi_mode;
                true
            }
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        let window_style = format!("left:{}px; top:{}px; z-index:{};",
            self.props.geometry.position.x,
            self.props.geometry.position.y,
            self.props.geometry.z_index);

        html! {
            <div class="module-window"
                style={window_style}
                ref={self.props.refs.module.clone()}
                onmousedown={stop_propagation()}
                oncontextmenu={stop_propagation()}
            >
                <div class="module-window-title"
                    onmousedown={self.link.callback(WindowMsg::DragStart)}
                    onmouseup={self.props.workspace.callback(WorkspaceMsg::MouseUp)}
                >
                    <div class="module-window-title-label">
                        {&self.props.name}
                    </div>
                    {self.view_custom_title_buttons()}
                    <div class="module-window-title-button module-window-title-delete" onmousedown={self.link.callback(|_| WindowMsg::Delete)}>
                        {"×"}
                    </div>
                </div>
                <div class="module-window-content">
                    <div class="module-window-inputs">
                        {self.view_inputs()}
                    </div>
                    <div class="module-window-params">
                        {self.view_params()}
                    </div>
                    <div class="module-window-outputs">
                        {self.view_outputs()}
                    </div>
                </div>
            </div>
        }
    }
}

impl Window {
    fn view_custom_title_buttons(&self) -> Html {
        match &self.props.module {
            ModuleParams::EqThree(..) |
            ModuleParams::Mixer(..) => {
                let class = match self.midi_mode {
                    MidiUiMode::Normal =>
                        "module-window-title-button module-window-title-midi-btn",
                    MidiUiMode::Configure =>
                        "module-window-title-button module-window-title-midi-btn module-window-title-midi-btn-active",
                };

                let new_midi_mode = match self.midi_mode {
                    MidiUiMode::Normal => MidiUiMode::Configure,
                    MidiUiMode::Configure => MidiUiMode::Normal,
                };

                html! {
                    <div class={class} onmousedown={self.link.callback(move |_| WindowMsg::SetMidiMode(new_midi_mode))}>
                        {"MIDI"}
                    </div>
                }
            }
            _ => html! {},
        }
    }
    fn view_inputs(&self) -> Html {
        self.view_terminals(
            self.props.refs.inputs.iter()
                .cloned()
                .enumerate()
                .map(|(index, terminal_ref)|
                    (TerminalId::Input(InputId(self.props.id, index)), terminal_ref)))
    }

    fn view_outputs(&self) -> Html {
        self.view_terminals(
            self.props.refs.outputs.iter()
                .cloned()
                .enumerate()
                .map(|(index, terminal_ref)|
                    (TerminalId::Output(OutputId(self.props.id, index)), terminal_ref)))
    }

    fn view_terminals(&self, terminals: impl Iterator<Item = (TerminalId, TerminalRef)>) -> Html {
        html! {
            { for terminals.map(|(terminal_id, terminal_ref)| {
                html! {
                    <Terminal
                        terminal={terminal_ref.clone()}
                        onmousedown={self.link.callback({
                            let terminal_ref = terminal_ref.clone();
                            move |ev| WindowMsg::TerminalMouseDown(ev, terminal_id, terminal_ref.clone())
                        })}
                    />
                }
            }) }
        }
    }

    fn view_params(&self) -> Html {
        match &self.props.module {
            ModuleParams::Oscillator(params) => {
                html! { <Oscillator id={self.props.id} module={self.link.clone()} params={params} /> }
            }
            ModuleParams::StereoPanner(()) |
            ModuleParams::StereoSplitter(()) => {
                html! {}
            }
            ModuleParams::OutputDevice(params) => {
                if let Some(Indication::OutputDevice(indication)) = &self.props.indication {
                    html! { <OutputDevice id={self.props.id} module={self.link.clone()} params={params} indication={indication} /> }
                } else {
                    unreachable!()
                }
            }
            ModuleParams::Plotter(_) => {
                if let Some(Indication::Plotter(indication)) = &self.props.indication {
                    html! { <Plotter id={self.props.id} indication={indication} /> }
                } else {
                    unreachable!()
                }
            }
            ModuleParams::FmSine(params) => {
                html! { <FmSine id={self.props.id} module={self.link.clone()} params={params} midi_mode={self.midi_mode} /> }
            }
            ModuleParams::Amplifier(params) => {
                html! { <Amplifier id={self.props.id} module={self.link.clone()} params={params} midi_mode={self.midi_mode} /> }
            }
            ModuleParams::Trigger(params) => {
                html! { <Trigger id={self.props.id} module={self.link.clone()} params={params} /> }
            }
            ModuleParams::Envelope(params) => {
                html! { <Envelope id={self.props.id} module={self.link.clone()} params={params} /> }
            }
            ModuleParams::Mixer(params) => {
                html! { <Mixer id={self.props.id} module={self.link.clone()} params={params} midi_mode={self.midi_mode} /> }
            }
            ModuleParams::StreamInput(params) => {
                html! { <StreamInput id={self.props.id} module={self.link.clone()} params={params} /> }
            }
            ModuleParams::StreamOutput(params) => {
                if let Some(Indication::StreamOutput(indication)) = &self.props.indication {
                    html! { <StreamOutput id={self.props.id} module={self.link.clone()} params={params} indication={indication} /> }
                } else {
                    unreachable!()
                }
            }
            ModuleParams::EqThree(params) => {
                html! { <EqThree id={self.props.id} module={self.link.clone()} params={params} midi_mode={self.midi_mode} /> }
            }
            ModuleParams::Monitor(()) => {
                if let Some(Indication::Monitor(indication)) = &self.props.indication {
                    html! { <Monitor id={self.props.id} indication={indication} /> }
                } else {
                    unreachable!()
                }
            }
            ModuleParams::VideoMixer(params) => {
                html! { <VideoMixer id={self.props.id} module={self.link.clone()} params={params} midi_mode={self.midi_mode} /> }
            }
            ModuleParams::MediaSource(params) => {
                html! { <MediaSource id={self.props.id} module={self.link.clone()} params={params} session={self.props.session.clone()} /> }
            }
            ModuleParams::Shader(_) => {
                html! {}
            }
        }
    }
}

pub struct Terminal {
    link: ComponentLink<Self>,
    props: TerminalProps,
    hover: bool,
}

#[derive(Properties, Clone, Debug)]
pub struct TerminalProps {
    terminal: TerminalRef,
    onmousedown: Callback<MouseEvent>,
}

impl Component for Terminal {
    type Properties = TerminalProps;
    type Message = bool;

    fn create(props: TerminalProps, link: ComponentLink<Self>) -> Self {
        Terminal { link, props, hover: false }
    }

    fn change(&mut self, props: TerminalProps) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        self.hover = msg;
        true
    }

    fn view(&self) -> Html {
        let class = format!(
            "module-window-terminal module-window-terminal-{}",
            self.props.terminal.terminal_type.to_css_name()
        );
        html! {
            <div
                class={class}
                ref={self.props.terminal.node.clone()}
                onmousedown={self.props.onmousedown.clone()}
                onmouseover={self.link.callback(|_| true)}
                onmouseout={self.link.callback(|_| false)}
                oncontextmenu={prevent_default()}
            >
                <div class="terminal-label">
                    {format!("{}", &self.props.terminal.label.as_ref().unwrap_or(&"".to_string()))}
                </div>

                <svg width="16" height="16">
                    { match self.props.terminal.line_type {
                        LineType::Mono => html! {},
                        LineType::Stereo => html! {
                            <polygon points="0,16 16,16 16,0" fill={ if self.hover { "#f0b5b3" } else { "#e0a5a3" } } />
                        },
                        LineType::Video => html! {
                            <rect width="16" height="16" fill={ if self.hover { "#fef8e1" } else { "#fdf1bf" } } />
                        }
                    } }
                </svg>
            </div>
        }
    }
}

pub struct Connections {
    canvas: NodeRef,
    props: ConnectionsProps,
}

#[derive(Properties, Clone, PartialEq, Eq)]
pub struct ConnectionsProps {
    connections: Vec<(Coords, Coords)>,
}

impl Component for Connections {
    type Message = ();
    type Properties = ConnectionsProps;

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Connections {
            canvas: NodeRef::default(),
            props,
        }
    }

    fn view(&self) -> Html {
        html! {
            <canvas
                class="workspace-connections"
                ref={self.canvas.clone()}
            />
        }
    }

    fn update(&mut self, _: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        if self.props.connections != props.connections {
            self.props.connections = props.connections;
            true
        } else {
            false
        }
    }

    fn rendered(&mut self, _: bool) {
        use std::cmp::max;

        if let Some(canvas) = self.canvas.cast::<HtmlCanvasElement>() {
            let ctx = canvas.get_context("2d")
                .expect("canvas.get_context")
                .expect("canvas.get_context");

            let ctx = ctx
                .dyn_into::<CanvasRenderingContext2d>()
                .expect("dyn_ref::<CanvasRenderingContext2d>");

            // plan multi-segment lines for all connections
            let lines = self.props.connections.iter()
                .map(|(a, b)| plan_line_points(*a, *b))
                .collect::<Vec<_>>();

            // calculate required canvas size for all points
            let Coords { x: width, y: height } = lines.iter()
                .flat_map(|segments| segments)
                .fold(Coords { x: 0, y: 0 }, |area, point| {
                    Coords {
                        x: max(area.x, point.x),
                        y: max(area.y, point.y),
                    }
                });

            canvas.set_width(width as u32 + 1);
            canvas.set_height(height as u32 + 1);

            // draw lines
            ctx.clear_rect(0f64, 0f64, width as f64, height as f64);

            for points in lines {
                ctx.begin_path();

                ctx.move_to(points[0].x as f64, points[0].y as f64);

                for point in &points[1..] {
                    ctx.line_to(point.x as f64, point.y as f64);
                }

                ctx.stroke();
            }
        }
    }
}

fn plan_line_points(start: Coords, end: Coords) -> Vec<Coords> {
    let mut segments = vec![];

    const END_SEGMENT_SIZE: Coords = Coords { x: 16, y: 0 };
    let effective_start = start.add(END_SEGMENT_SIZE);
    let effective_end = end.sub(END_SEGMENT_SIZE);

    segments.push(start);
    segments.push(effective_start);

    if effective_start.x <= effective_end.x {
        // line is mostly horizontal:
        let x_midpoint = (effective_start.x + effective_end.x) / 2;

        segments.push(Coords { x: x_midpoint, y: effective_start.y });
        segments.push(Coords { x: x_midpoint, y: effective_end.y });
    } else {
        // line is mostly vertical:
        let y_midpoint = (effective_start.y + effective_end.y) / 2;

        segments.push(Coords { x: effective_start.x, y: y_midpoint });
        segments.push(Coords { x: effective_end.x, y: y_midpoint });
    }

    segments.push(effective_end);
    segments.push(end);

    segments
}
