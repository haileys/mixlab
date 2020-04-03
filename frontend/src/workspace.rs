use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::mem;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlElement, HtmlCanvasElement, MouseEvent};
use yew::{html, Callback, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

use mixlab_protocol::{ModuleId, TerminalId, InputId, OutputId, ModuleParams, SineGeneratorParams, ClientMessage, WindowGeometry, Coords, Indication, OutputDeviceParams, FmSineParams, AmplifierParams, GateState, LineType};

use crate::module::trigger::Trigger;
use crate::module::amplifier::Amplifier;
use crate::module::fm_sine::FmSine;
use crate::module::output_device::OutputDevice;
use crate::module::sine_generator::SineGenerator;
use crate::util::{callback_ex, stop_propagation, prevent_default};
use crate::{App, AppMsg, State};

pub struct Counter(usize);

impl Counter {
    pub fn new() -> Self {
        Counter(0)
    }

    pub fn next(&mut self) -> usize {
        let seq = self.0;
        self.0 += 1;
        seq
    }
}

pub struct Workspace {
    link: ComponentLink<Self>,
    props: WorkspaceProps,
    gen_z_index: Counter,
    mouse: MouseMode,
    window_refs: BTreeMap<ModuleId, WindowRef>,
}

#[derive(Properties, Clone)]
pub struct WorkspaceProps {
    pub app: ComponentLink<App>,
    pub state: Rc<RefCell<State>>,
    pub state_seq: usize,
    pub width: usize,
    pub height: usize,
}

pub enum MouseMode {
    Normal,
    Drag(Drag),
    Connect(TerminalId, Option<Coords>),
    ContextMenu(Coords),
}

pub struct Drag {
    module: ModuleId,
    origin: Coords,
}

#[derive(Debug)]
pub enum WorkspaceMsg {
    DragStart(ModuleId, MouseEvent),
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    MouseMove(MouseEvent),
    SelectTerminal(TerminalId),
    ClearTerminal(TerminalId),
    DeleteWindow(ModuleId),
    UpdateModuleParams(ModuleId, ModuleParams),
    CreateModule(ModuleParams, Coords),
}

impl Component for Workspace {
    type Message = WorkspaceMsg;
    type Properties = WorkspaceProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let state = props.state.clone();

        let mut workspace = Workspace {
            link,
            props,
            gen_z_index: Counter::new(),
            mouse: MouseMode::Normal,
            window_refs: BTreeMap::new(),
        };

        let state = state.borrow();
        for id in state.modules.keys() {
            let inputs = state.inputs.get(id);
            let outputs = state.outputs.get(id);

            if let (Some(inputs), Some(outputs)) = (inputs, outputs) {
                workspace.register_terminals(*id, inputs, outputs);
            }
        }

        workspace
    }

    fn change(&mut self, new_props: Self::Properties) -> ShouldRender {
        let mut should_render = false;

        let mut deleted_windows = self.window_refs.keys().copied().collect::<HashSet<_>>();

        {
            let state = new_props.state.borrow();

            for id in state.modules.keys() {
                if deleted_windows.remove(id) {
                    // cool, nothing changes with this module
                } else {
                    // this module was not present before, create a window ref for it
                    let inputs = state.inputs.get(id);
                    let outputs = state.outputs.get(id);

                    if let (Some(inputs), Some(outputs)) = (inputs, outputs) {
                        self.register_terminals(*id, inputs, outputs);
                    }

                    should_render = true;
                }
            }
        }

        for deleted_window in deleted_windows {
            self.window_refs.remove(&deleted_window);
            should_render = true;
        }

        if self.props.state_seq != new_props.state_seq {
            should_render = true;
        }

        if self.props.width != new_props.width {
            should_render = true;
        }

        if self.props.height != new_props.height {
            should_render = true;
        }

        self.props = new_props;

        should_render
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        return match msg {
            WorkspaceMsg::DragStart(module, ev) => {
                let mut state = self.props.state.borrow_mut();

                if let Some(geom) = state.geometry.get_mut(&module) {
                    self.mouse = MouseMode::Drag(Drag {
                        module,
                        origin: Coords { x: ev.page_x(), y: ev.page_y() },
                    });

                    geom.z_index = self.gen_z_index.next();

                    true
                } else {
                    false
                }
            }
            WorkspaceMsg::MouseDown(ev) => {
                const RIGHT_MOUSE_BUTTON: u16 = 2;

                crate::log!("WorkspaceMsg::MouseDown: buttons: {}", ev.buttons());

                if (ev.buttons() & RIGHT_MOUSE_BUTTON) != 0 {
                    match self.mouse {
                        MouseMode::Connect(..) => {
                            self.mouse = MouseMode::Normal;
                        }
                        MouseMode::Normal | MouseMode::ContextMenu(_) => {
                            self.mouse = MouseMode::ContextMenu(Coords { x: ev.page_x(), y: ev.page_y() });
                        }
                        MouseMode::Drag(_) => {}
                    }

                    ev.prevent_default();
                    ev.stop_propagation();

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
                                    ClientMessage::UpdateWindowGeometry(drag.module, geometry.clone())));
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
                    MouseMode::Connect(_terminal, ref mut coords) => {
                        *coords = Some(Coords { x: ev.page_x(), y: ev.page_y() });
                        true
                    }
                }
            }
            WorkspaceMsg::SelectTerminal(terminal) => {
                match &self.mouse {
                    MouseMode::Normal | MouseMode::ContextMenu(_) => {
                        self.mouse = MouseMode::Connect(terminal, None);
                        false
                    }
                    MouseMode::Connect(other_terminal, _) => {
                        match (terminal, *other_terminal) {
                            (TerminalId::Input(input), TerminalId::Output(output)) |
                            (TerminalId::Output(output), TerminalId::Input(input)) => {
                                self.props.state.borrow_mut()
                                    .connections
                                    .insert(input, output);

                                self.mouse = MouseMode::Normal;

                                self.props.app.send_message(
                                    AppMsg::ClientUpdate(
                                        ClientMessage::CreateConnection(input, output)));

                                true
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
                                ClientMessage::DeleteConnection(input)));
                    }
                    TerminalId::Output(output) => {
                        let mut msgs = Vec::new();

                        let mut state = self.props.state.borrow_mut();

                        for (in_, out_) in &state.connections {
                            if *out_ == output {
                                msgs.push(AppMsg::ClientUpdate(
                                    ClientMessage::DeleteConnection(*in_)));
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
                        ClientMessage::DeleteModule(module)));

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
                                ClientMessage::UpdateModuleParams(module, params)));

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
                    z_index: self.gen_z_index.next(),
                };

                self.props.app.send_message(
                    AppMsg::ClientUpdate(
                        ClientMessage::CreateModule(module, geometry)));

                true
            }
        };

        fn drag_event(state: &mut State, window_refs: &BTreeMap<ModuleId, WindowRef>, drag: &mut Drag, ev: MouseEvent) -> ShouldRender {
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

        crate::log!("view: connections: {:?}", connections);

        if let MouseMode::Connect(terminal, Some(to_coords)) = &self.mouse {
            if let Some(start_coords) = self.screen_coords_for_terminal(*terminal) {
                let pair = match terminal {
                    TerminalId::Input(_) => (*to_coords, start_coords),
                    TerminalId::Output(_) => (start_coords, *to_coords),
                };

                connections.push(pair);
            }
        }

        html! {
            <>
                <div class="workspace"
                    onmouseup={self.link.callback(WorkspaceMsg::MouseUp)}
                    onmousemove={self.link.callback(WorkspaceMsg::MouseMove)}
                    onmousedown={self.link.callback(WorkspaceMsg::MouseDown)}
                    oncontextmenu={prevent_default()}
                >
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
                            /> }
                        } else {
                            html! {}
                        }
                    }) }

                    <Connections connections={connections} width={self.props.width} height={self.props.height} />

                    {self.view_context_menu()}
                </div>
            </>
        }
    }

    fn mounted(&mut self) -> ShouldRender {
        // always re-render after first mount because rendering correctly
        // requires noderefs
        true
    }
}

impl Workspace {
    pub fn register_terminals(&mut self, id: ModuleId, inputs: &[LineType], outputs: &[LineType]) {
        let refs = WindowRef {
            module: NodeRef::default(),
            inputs: make_terminal_refs(inputs),
            outputs: make_terminal_refs(outputs),
        };

        self.window_refs.insert(id, refs);

        fn make_terminal_refs(line_types: &[LineType]) -> Vec<TerminalRef> {
            line_types.iter()
                .cloned()
                .map(|line_type| TerminalRef {
                    node: NodeRef::default(),
                    line_type,
                })
                .collect()
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
            ("Sine Generator", ModuleParams::SineGenerator(SineGeneratorParams { freq: 100.0 })),
            ("Mixer (2 channel)", ModuleParams::Mixer2ch(())),
            ("Output Device", ModuleParams::OutputDevice(OutputDeviceParams { device: None, left: None, right: None })),
            ("FM Sine", ModuleParams::FmSine(FmSineParams { freq_lo: 90.0, freq_hi: 110.0 })),
            ("Amplifier", ModuleParams::Amplifier(AmplifierParams { amplitude: 1.0, mod_depth: 0.5 })),
            ("Trigger", ModuleParams::Trigger(GateState::Closed)),
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
}

#[derive(Debug)]
pub enum WindowMsg {
    DragStart(MouseEvent),
    TerminalMouseDown(MouseEvent, TerminalId),
    Delete,
    UpdateParams(ModuleParams),
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
}

#[derive(Clone, Debug)]
pub struct WindowRef {
    module: NodeRef,
    inputs: Vec<TerminalRef>,
    outputs: Vec<TerminalRef>,
}

#[derive(Clone, Debug)]
pub struct TerminalRef {
    node: NodeRef,
    line_type: LineType,
}

impl Component for Window {
    type Message = WindowMsg;
    type Properties = WindowProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Window {
            link,
            props,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            WindowMsg::DragStart(ev) => {
                ev.stop_propagation();

                self.props.workspace.send_message(
                    WorkspaceMsg::DragStart(self.props.id, ev));
            }
            WindowMsg::TerminalMouseDown(ev, terminal_id) => {
                let msg =
                    if (ev.buttons() & 2) != 0 {
                        // right click
                        WorkspaceMsg::ClearTerminal(terminal_id)
                    } else {
                        WorkspaceMsg::SelectTerminal(terminal_id)
                    };

                self.props.workspace.send_message(msg);

                ev.stop_propagation();
            }
            WindowMsg::Delete => {
                self.props.workspace.send_message(
                    WorkspaceMsg::DeleteWindow(self.props.id));
            }
            WindowMsg::UpdateParams(params) => {
                self.props.workspace.send_message(
                    WorkspaceMsg::UpdateModuleParams(self.props.id, params));
            }
        }
        false
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
                oncontextmenu={prevent_default()}
            >
                <div class="module-window-title"
                    onmousedown={callback_ex(&self.link, WindowMsg::DragStart)}
                >
                    <div class="module-window-title-label">
                        {&self.props.name}
                    </div>
                    <div class="module-window-title-delete" onmousedown={self.link.callback(|_| WindowMsg::Delete)}>
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
    fn view_inputs(&self) -> Html {
        html! {
            { for self.props.refs.inputs.iter().cloned().enumerate().map(|(index, terminal_ref)| {
                let terminal_id = TerminalId::Input(InputId(self.props.id, index));

                html! {
                    <Terminal
                        terminal={terminal_ref}
                        onmousedown={self.link.callback(move |ev| WindowMsg::TerminalMouseDown(ev, terminal_id))}
                    />
                }
            }) }
        }
    }

    fn view_outputs(&self) -> Html {
        html! {
            { for self.props.refs.outputs.iter().cloned().enumerate().map(|(index, terminal_ref)| {
                let terminal_id = TerminalId::Output(OutputId(self.props.id, index));

                html! {
                    <Terminal
                        terminal={terminal_ref}
                        onmousedown={self.link.callback(move |ev| WindowMsg::TerminalMouseDown(ev, terminal_id))}
                    />
                }
            }) }
        }
    }

    fn view_params(&self) -> Html {
        match &self.props.module {
            ModuleParams::SineGenerator(params) => {
                html! { <SineGenerator id={self.props.id} module={self.link.clone()} params={params} /> }
            }
            ModuleParams::Mixer2ch(_) => {
                html! {}
            }
            ModuleParams::OutputDevice(params) => {
                if let Some(Indication::OutputDevice(indic)) = &self.props.indication {
                    html! { <OutputDevice id={self.props.id} module={self.link.clone()} params={params} indication={indic} /> }
                } else {
                    html! {}
                }
            }
            ModuleParams::FmSine(params) => {
                html! { <FmSine id={self.props.id} module={self.link.clone()} params={params} /> }
            }
            ModuleParams::Amplifier(params) => {
                html! { <Amplifier id={self.props.id} module={self.link.clone()} params={params} /> }
            }
            ModuleParams::Trigger(params) => {
                html! { <Trigger id={self.props.id} module={self.link.clone()} params={params} /> }
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
        html! {
            <div class="module-window-terminal"
                ref={self.props.terminal.node.clone()}
                onmousedown={self.props.onmousedown.clone()}
                onmouseover={self.link.callback(|_| true)}
                onmouseout={self.link.callback(|_| false)}
            >
                <svg width="16" height="16">
                    { match self.props.terminal.line_type {
                        LineType::Stereo => html! {
                            <polygon points="0,16 16,16 16,0" fill={ if self.hover { "#f0b5b3" } else { "#e0a5a3" } } />
                        }
                    } }
                </svg>
            </div>
        }
    }
}

pub struct Connections {
    canvas: NodeRef,
    ctx: Option<RefCell<CanvasRenderingContext2d>>,
    props: ConnectionsProps,
}

#[derive(Properties, Clone)]
pub struct ConnectionsProps {
    width: usize,
    height: usize,
    connections: Vec<(Coords, Coords)>,
}

impl Component for Connections {
    type Message = ();
    type Properties = ConnectionsProps;

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Connections {
            canvas: NodeRef::default(),
            ctx: None,
            props,
        }
    }

    fn view(&self) -> Html {
        if let Some(ref ctx) = self.ctx {
            let ctx = ctx.borrow_mut();

            ctx.clear_rect(0f64, 0f64, self.props.width as f64, self.props.height as f64);

            for conn in &self.props.connections {
                ctx.begin_path();

                let points = plan_line_points(conn.0, conn.1);

                ctx.move_to(points[0].x as f64, points[0].y as f64);

                for point in &points[1..] {
                    ctx.line_to(point.x as f64, point.y as f64);
                }

                ctx.stroke();
            }
        }

        html! {
            <canvas
                /*onmousedown={self.link.callback(|ev| ConnectionsMsg::MouseDown(ev))}*/
                class="workspace-connections"
                width={self.props.width}
                height={self.props.height}
                ref={self.canvas.clone()}
            />
        }
    }

    fn update(&mut self, _: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn mounted(&mut self) -> ShouldRender {
        if let Some(canvas) = self.canvas.cast::<HtmlCanvasElement>() {
            let ctx = canvas.get_context("2d")
                .expect("canvas.get_context")
                .expect("canvas.get_context");

            let ctx = ctx
                .dyn_into::<CanvasRenderingContext2d>()
                .expect("dyn_ref::<CanvasRenderingContext2d>");

            self.ctx = Some(RefCell::new(ctx));
        }

        true
    }

    // fn draw_connections(&self, )
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
