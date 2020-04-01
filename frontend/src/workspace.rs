use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlElement, HtmlCanvasElement, MouseEvent};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

pub struct Counter(usize);

impl Counter {
    pub fn new() -> Self {
        Counter(0)
    }

    pub fn next(&mut self) -> usize {
        let num = self.0;
        self.0 += 1;
        num
    }
}

type WindowSet = BTreeMap<WindowId, WindowProps>;

pub struct Workspace {
    link: ComponentLink<Self>,
    gen_id: Counter,
    gen_z_index: Counter,
    windows: WindowSet,
    mouse: MouseMode,
    connections: HashMap<InputId, OutputId>,
}

pub enum MouseMode {
    Normal,
    Drag(Drag),
    Connect(TerminalId, Option<Coords>),
}

pub struct Drag {
    window: WindowId,
    origin: Coords,
}

#[derive(Clone, Copy, Debug)]
pub struct Coords {
    x: i32,
    y: i32,
}

impl Coords {
    pub fn add(&self, other: Coords) -> Coords {
        Coords {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    pub fn sub(&self, other: Coords) -> Coords {
        Coords {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    pub fn to_css(&self) -> String {
        format!("left:{}px; top:{}px;", self.x, self.y)
    }
}

pub enum WorkspaceMsg {
    DragStart(WindowId, MouseEvent),
    ContextMenu(MouseEvent),
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    MouseMove(MouseEvent),
    SelectTerminal(TerminalId),
    ClearTerminal(TerminalId),
    DeleteWindow(WindowId),
}

impl Component for Workspace {
    type Message = WorkspaceMsg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut workspace = Workspace {
            link,
            gen_id: Counter::new(),
            gen_z_index: Counter::new(),
            windows: BTreeMap::new(),
            mouse: MouseMode::Normal,
            connections: HashMap::new(),
        };

        workspace.create_window();
        workspace.create_window();
        workspace.create_window();
        workspace.create_window();

        workspace
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        return match msg {
            WorkspaceMsg::DragStart(window, ev) => {
                if let Some(props) = self.windows.get_mut(&window) {
                    self.mouse = MouseMode::Drag(Drag {
                        window,
                        origin: Coords { x: ev.page_x(), y: ev.page_y() },
                    });

                    props.z_index = self.gen_z_index.next();

                    true
                } else {
                    false
                }
            }
            WorkspaceMsg::ContextMenu(ev) => {
                ev.prevent_default();
                false
            }
            WorkspaceMsg::MouseDown(ev) => {
                const RIGHT_MOUSE_BUTTON: u16 = 2;

                crate::log!("buttons: {}", ev.buttons());

                if (ev.buttons() & RIGHT_MOUSE_BUTTON) != 0 {
                    if let MouseMode::Connect(..) = self.mouse {
                        self.mouse = MouseMode::Normal;
                    }

                    ev.prevent_default();
                    ev.stop_immediate_propagation();

                    true
                } else {
                    false
                }
            }
            WorkspaceMsg::MouseUp(ev) => {
                match self.mouse {
                    MouseMode::Normal => false,
                    MouseMode::Drag(ref mut drag) => {
                        let should_render = drag_event(&mut self.windows, drag, ev);
                        self.mouse = MouseMode::Normal;
                        should_render
                    }
                    MouseMode::Connect(..) => false,
                }
            }
            WorkspaceMsg::MouseMove(ev) => {
                match &mut self.mouse {
                    MouseMode::Normal => false,
                    MouseMode::Drag(ref mut drag) => {
                        drag_event(&mut self.windows, drag, ev)
                    }
                    MouseMode::Connect(_terminal, ref mut coords) => {
                        *coords = Some(Coords { x: ev.page_x(), y: ev.page_y() });
                        true
                    }
                }
            }
            WorkspaceMsg::SelectTerminal(terminal) => {
                match &self.mouse {
                    MouseMode::Normal => {
                        self.mouse = MouseMode::Connect(terminal, None);
                        false
                    }
                    MouseMode::Connect(other_terminal, _) => {
                        match (terminal, *other_terminal) {
                            (TerminalId::Input(input), TerminalId::Output(output)) |
                            (TerminalId::Output(output), TerminalId::Input(input)) => {
                                self.connections.insert(input, output);
                                self.mouse = MouseMode::Normal;
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
                        self.connections.remove(&input);
                    }
                    TerminalId::Output(output) => {
                        self.connections.retain(|_, out| output != *out);
                    }
                }
                true
            }
            WorkspaceMsg::DeleteWindow(window) => {
                self.windows.remove(&window);
                self.connections.retain(|input, output| {
                    output.window_id() != window && input.window_id() != window
                });
                true
            }
        };

        fn drag_event(windows: &mut WindowSet, drag: &mut Drag, ev: MouseEvent) -> ShouldRender {
            let mouse_pos = Coords { x: ev.page_x(), y: ev.page_y() };

            let delta = mouse_pos.sub(drag.origin);
            drag.origin = mouse_pos;

            if let Some(props) = windows.get_mut(&drag.window) {
                props.position = props.position.add(delta);

                if let Some(el) = props.refs.window.cast::<HtmlElement>() {
                    let style = el.style();
                    let _ = style.set_property("left", &format!("{}px", props.position.x));
                    let _ = style.set_property("top", &format!("{}px", props.position.y));
                }

                true
            } else {
                false
            }
        }
    }

    fn view(&self) -> Html {
        let mut connections: Vec<(Coords, Coords)> = vec![];

        for (input, output) in &self.connections {
            if let Some(input_coords) = self.screen_coords_for_terminal(TerminalId::Input(*input)) {
                if let Some(output_coords) = self.screen_coords_for_terminal(TerminalId::Output(*output)) {
                    connections.push((output_coords, input_coords));
                }
            }
        }

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
                    oncontextmenu={self.link.callback(WorkspaceMsg::ContextMenu)}
                >
                    { for self.windows.values().cloned().map(|props|
                        html! { <div data-window-id={props.id.0}><Window with props /></div> }) }
                </div>
                <Connections connections={connections} width={1000} height={1000} />
            </>
        }
    }
}

impl Workspace {
    pub fn create_window(&mut self) {
        let id = WindowId(self.gen_id.next());

        let kind = match id.0 {
            0 => WindowKind::SineGenerator,
            1 => WindowKind::SineGenerator,
            2 => WindowKind::OutputDevice,
            3 => WindowKind::Mixer2ch,
            _ => unreachable!(),
        };

        let refs = WindowRef {
            window: NodeRef::default(),
            inputs: match kind {
                WindowKind::SineGenerator => vec![],
                WindowKind::OutputDevice => vec![NodeRef::default()],
                WindowKind::Mixer2ch => vec![NodeRef::default(), NodeRef::default()],
            },
            outputs: match kind {
                WindowKind::SineGenerator => vec![NodeRef::default()],
                WindowKind::OutputDevice => vec![],
                WindowKind::Mixer2ch => vec![NodeRef::default()],
            },
        };

        let props = WindowProps {
            id: id,
            kind,
            refs,
            name: format!("{:?}", kind),
            workspace: self.link.clone(),
            position: Coords {
                x: (id.0 as i32 + 1) * 100,
                y: (id.0 as i32 + 1) * 100,
            },
            z_index: self.gen_z_index.next(),
        };

        self.windows.insert(id, props);
    }

    fn screen_coords_for_terminal(&self, terminal: TerminalId) -> Option<Coords> {
        let window_props = self.windows.get(&terminal.window_id())?;

        let terminal_node = match terminal {
            TerminalId::Input(InputId(_, index)) => window_props.refs.inputs.get(index)?,
            TerminalId::Output(OutputId(_, index)) => window_props.refs.outputs.get(index)?,
        };

        let terminal_node = terminal_node.cast::<HtmlElement>()?;

        let terminal_coords = Coords { x: terminal_node.offset_left() + 9, y: terminal_node.offset_top() + 9 };
        Some(window_props.position.add(terminal_coords))
    }
}

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct WindowId(usize);

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum TerminalId {
    Input(InputId),
    Output(OutputId),
}

impl TerminalId {
    pub fn window_id(&self) -> WindowId {
        match self {
            TerminalId::Input(input) => input.window_id(),
            TerminalId::Output(output) => output.window_id(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct InputId(WindowId, usize);

impl InputId {
    pub fn window_id(&self) -> WindowId {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct OutputId(WindowId, usize);

impl OutputId {
    pub fn window_id(&self) -> WindowId {
        self.0
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
    TerminalContextMenu(MouseEvent),
    Delete,
}

#[derive(Properties, Clone, Debug)]
pub struct WindowProps {
    pub id: WindowId,
    pub kind: WindowKind,
    pub name: String,
    pub workspace: ComponentLink<Workspace>,
    pub position: Coords,
    pub z_index: usize,
    pub refs: WindowRef,
}

#[derive(Clone, Debug)]
pub struct WindowRef {
    window: NodeRef,
    inputs: Vec<NodeRef>,
    outputs: Vec<NodeRef>,
}

#[derive(Debug, Clone, Copy)]
pub enum WindowKind {
    SineGenerator,
    OutputDevice,
    Mixer2ch,
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
            WindowMsg::TerminalContextMenu(ev) => {
                ev.stop_propagation();
            }
            WindowMsg::Delete => {
                self.props.workspace.send_message(
                    WorkspaceMsg::DeleteWindow(self.props.id));
            }
        }
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        let mut window_style = self.props.position.to_css();
        let _ = write!(&mut window_style, "z-index:{};", self.props.z_index);

        html! {
            <div class="module-window" style={window_style} ref={self.props.refs.window.clone()}>
                <div class="module-window-title"
                    onmousedown={self.link.callback(|ev: MouseEvent| WindowMsg::DragStart(ev))}
                    oncontextmenu={self.link.callback(WindowMsg::TerminalContextMenu)}
                >
                    <div class="module-window-title-label">
                        {&self.props.name}
                    </div>
                    <div class="module-window-title-delete" onmousedown={self.link.callback(|_| WindowMsg::Delete)}>
                        {"×"}
                    </div>
                </div>
                <div class="module-window-content">
                    {self.view_inputs()}
                    {self.view_outputs()}
                </div>
            </div>
        }
    }
}

impl Window {
    fn view_inputs(&self) -> Html {
        html! {
            <div class="module-window-inputs">
                { for self.props.refs.inputs.iter().cloned().enumerate().map(|(index, terminal_ref)| {
                    let terminal_id = TerminalId::Input(InputId(self.props.id, index));

                    html! {
                        <div class="module-window-terminal"
                            ref={terminal_ref}
                            onmousedown={self.link.callback(move |ev| WindowMsg::TerminalMouseDown(ev, terminal_id))}
                        >
                        </div>
                    }
                }) }
            </div>
        }
    }

    fn view_outputs(&self) -> Html {
        html! {
            <div class="module-window-outputs">
                { for self.props.refs.outputs.iter().cloned().enumerate().map(|(index, terminal_ref)| {
                    let terminal_id = TerminalId::Output(OutputId(self.props.id, index));

                    html! {
                        <div class="module-window-terminal"
                            ref={terminal_ref}
                            onmousedown={self.link.callback(move |ev| WindowMsg::TerminalMouseDown(ev, terminal_id))}
                        >
                        </div>
                    }
                }) }
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
