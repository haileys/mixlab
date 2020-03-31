use std::cell::RefCell;
use std::cmp;
use std::collections::BTreeMap;
use std::convert::{TryInto, TryFrom};
use std::fmt::Write;

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Element, HtmlElement, HtmlCanvasElement, MouseEvent};
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

type WindowSet = BTreeMap<WindowId, (NodeRef, WindowProps)>;

pub struct Workspace {
    link: ComponentLink<Self>,
    gen_id: Counter,
    gen_z_index: Counter,
    windows: WindowSet,
    mouse: MouseMode,
    connections: Vec<(WindowId, NodeRef, WindowId, NodeRef)>,
}

pub enum MouseMode {
    Normal,
    Drag(Drag),
    Connect(WindowId, NodeRef, Option<Coords>),
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
    SelectTerminal(WindowId, NodeRef),
}

impl Component for Workspace {
    type Message = WorkspaceMsg;
    type Properties = ();

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut workspace = Workspace {
            link,
            gen_id: Counter::new(),
            gen_z_index: Counter::new(),
            windows: BTreeMap::new(),
            mouse: MouseMode::Normal,
            connections: Vec::new(),
        };

        workspace.create_window();
        workspace.create_window();

        workspace
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        return match msg {
            WorkspaceMsg::DragStart(window, ev) => {
                if let Some((_node, props)) = self.windows.get_mut(&window) {
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
                    MouseMode::Connect(_window, _node, ref mut coords) => {
                        *coords = Some(Coords { x: ev.page_x(), y: ev.page_y() });
                        true
                    }
                }
            }
            WorkspaceMsg::SelectTerminal(window, node) => {
                match &self.mouse {
                    MouseMode::Normal => {
                        self.mouse = MouseMode::Connect(window, node, None);
                    }
                    MouseMode::Connect(other_window, other_node, _) => {
                        // don't let user connect a node to itself
                        if other_node != &node {
                            self.connections.push((window, node, *other_window, other_node.clone()));
                            self.mouse = MouseMode::Normal;
                        }
                    }
                    MouseMode::Drag(_) => {}
                }

                false
            }
        };

        fn drag_event(windows: &mut WindowSet, drag: &mut Drag, ev: MouseEvent) -> ShouldRender {
            let mouse_pos = Coords { x: ev.page_x(), y: ev.page_y() };

            let delta = mouse_pos.sub(drag.origin);
            drag.origin = mouse_pos;

            if let Some((node, props)) = windows.get_mut(&drag.window) {
                props.position = props.position.add(delta);

                if let Some(el) = node.cast::<HtmlElement>() {
                    el.style().set_property("left", &format!("{}px", props.position.x));
                    el.style().set_property("top", &format!("{}px", props.position.y));
                }

                true
            } else {
                false
            }
        }
    }

    fn view(&self) -> Html {
        let mut connections: Vec<(Coords, Coords)> = vec![];

        for (from_window, from_node, to_window, to_node) in &self.connections {
            if let Some(from_coords) = self.screen_coords_for_terminal(*from_window, from_node) {
                if let Some(to_coords) = self.screen_coords_for_terminal(*to_window, to_node) {
                    connections.push((from_coords, to_coords));
                }
            }
        }

        if let MouseMode::Connect(window, start_node, Some(to_coords)) = &self.mouse {
            if let Some(start_coords) = self.screen_coords_for_terminal(*window, start_node) {
                connections.push((start_coords, *to_coords));
            }
        }

        html! {
            <div class="workspace"
                onmouseup={self.link.callback(WorkspaceMsg::MouseUp)}
                onmousemove={self.link.callback(WorkspaceMsg::MouseMove)}
                onmousedown={self.link.callback(WorkspaceMsg::MouseDown)}
                oncontextmenu={self.link.callback(WorkspaceMsg::ContextMenu)}
            >
                { for self.windows.values().cloned().map(|(ref_, props)|
                    html! { <Window with props ref={ref_} /> }) }

                <Connections connections={connections} width={1000} height={1000} />
            </div>
        }
    }
}

impl Workspace {
    pub fn create_window(&mut self) {
        let id = WindowId(self.gen_id.next());

        let kind = match id.0 {
            0 => WindowKind::SineGenerator,
            1 => WindowKind::OutputDevice,
            _ => unreachable!(),
        };

        let window = WindowProps {
            id: id,
            kind: kind,
            name: format!("{:?}", kind),
            workspace: self.link.clone(),
            position: Coords {
                x: (id.0 as i32 + 1) * 100,
                y: (id.0 as i32 + 1) * 100,
            },
            z_index: self.gen_z_index.next(),
        };

        self.windows.insert(id, (NodeRef::default(), window));
    }

    fn screen_coords_for_terminal(&self, window: WindowId, terminal_node: &NodeRef) -> Option<Coords> {
        let (_, window_props) = self.windows.get(&window)?;
        let terminal_node = terminal_node.cast::<HtmlElement>()?;
        let terminal_coords = Coords { x: terminal_node.offset_left() + 9, y: terminal_node.offset_top() + 9 };
        Some(window_props.position.add(terminal_coords))
    }
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct WindowId(usize);

pub struct Window {
    link: ComponentLink<Self>,
    props: WindowProps,
    terminal_noderef: NodeRef,
}

#[derive(Debug)]
pub enum WindowMsg {
    DragStart(MouseEvent),
    SelectTerminal(MouseEvent),
}

#[derive(Properties, Clone)]
pub struct WindowProps {
    pub id: WindowId,
    pub kind: WindowKind,
    pub name: String,
    pub workspace: ComponentLink<Workspace>,
    pub position: Coords,
    pub z_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum WindowKind {
    SineGenerator,
    OutputDevice,
}

impl Component for Window {
    type Message = WindowMsg;
    type Properties = WindowProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Window {
            link,
            props,
            terminal_noderef: NodeRef::default(),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            WindowMsg::DragStart(ev) => {
                self.props.workspace.send_message(
                    WorkspaceMsg::DragStart(self.props.id, ev));
            }
            WindowMsg::SelectTerminal(ev) => {
                self.props.workspace.send_message(
                    WorkspaceMsg::SelectTerminal(self.props.id, self.terminal_noderef.clone()));

                ev.stop_immediate_propagation();
            }
        }
        false
    }

    fn view(&self) -> Html {
        let mut window_style = self.props.position.to_css();
        let _ = write!(&mut window_style, "z-index:{};", self.props.z_index);

        html! {
            <div class="module-window" style={window_style}>
                <div class="module-window-title"
                    onmousedown={self.link.callback(|ev: MouseEvent| WindowMsg::DragStart(ev))}
                >
                    {&self.props.name}
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
        match self.props.kind {
            WindowKind::SineGenerator => html! {},
            WindowKind::OutputDevice => {
                html! {
                    <div class="module-window-inputs">
                        <div class="module-window-terminal"
                            ref={self.terminal_noderef.clone()}
                            onclick={self.link.callback(WindowMsg::SelectTerminal)}
                        >
                        </div>
                    </div>
                }
            }
        }
    }

    fn view_outputs(&self) -> Html {
        match self.props.kind {
            WindowKind::SineGenerator => {
                html! {
                    <div class="module-window-outputs">
                        <div class="module-window-terminal"
                            ref={self.terminal_noderef.clone()}
                            onclick={self.link.callback(WindowMsg::SelectTerminal)}
                        >
                        </div>
                    </div>
                }
            }
            WindowKind::OutputDevice => html! {},
        }
    }
}

pub struct Connections {
    link: ComponentLink<Self>,
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

#[derive(Debug)]
pub enum ConnectionsMsg {
    MouseDown(MouseEvent),
}

impl Component for Connections {
    type Message = ConnectionsMsg;
    type Properties = ConnectionsProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Connections {
            link,
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
                ctx.move_to(conn.0.x as f64, conn.0.y as f64);
                ctx.line_to(conn.1.x as f64, conn.1.y as f64);
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

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        crate::log!("{:?}", msg);

        match msg {
            ConnectionsMsg::MouseDown(ev) => { ev.prevent_default(); }
        }

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
