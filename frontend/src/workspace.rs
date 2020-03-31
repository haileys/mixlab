use std::collections::BTreeMap;
use std::convert::{TryInto, TryFrom};

use web_sys::{HtmlElement, MouseEvent, Element};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

pub struct Workspace {
    link: ComponentLink<Self>,
    next_id: usize,
    windows: BTreeMap<WindowId, (NodeRef, WindowProps)>,
    drag: Option<Drag>
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
    DragEnd(MouseEvent),
    Drag(MouseEvent),
}

impl Component for Workspace {
    type Message = WorkspaceMsg;
    type Properties = ();

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut workspace = Workspace {
            link,
            next_id: 0,
            windows: BTreeMap::new(),
            drag: None,
        };

        workspace.create_window();
        workspace.create_window();

        workspace
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            WorkspaceMsg::DragStart(window, ev) => {
                self.drag = Some(Drag {
                    window,
                    origin: Coords { x: ev.page_x(), y: ev.page_y() },
                });
            }
            WorkspaceMsg::DragEnd(ev) => {
                if let Some(drag) = self.drag.take() {
                    let mouse_pos = Coords { x: ev.page_x(), y: ev.page_y() };

                    let delta = mouse_pos.sub(drag.origin);

                    crate::log(&format!("would have moved {:?}", delta));

                    if let Some((node, props)) = self.windows.get_mut(&drag.window) {
                        props.position = props.position.add(delta);
                        if let Some(el) = node.cast::<HtmlElement>() {
                            el.style().set_property("left", &format!("{}px", props.position.x));
                            el.style().set_property("top", &format!("{}px", props.position.y));
                        }
                        return true;
                    }
                }
            }
            WorkspaceMsg::Drag(ev) => {
                if let Some(drag) = self.drag.as_mut() {
                    let mouse_pos = Coords { x: ev.page_x(), y: ev.page_y() };

                    let delta = mouse_pos.sub(drag.origin);
                    drag.origin = mouse_pos;

                    if let Some((node, props)) = self.windows.get_mut(&drag.window) {
                        props.position = props.position.add(delta);
                        if let Some(el) = node.cast::<HtmlElement>() {
                            el.style().set_property("left", &format!("{}px", props.position.x));
                            el.style().set_property("top", &format!("{}px", props.position.y));
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="workspace"
                onmouseup={self.link.callback(|ev: MouseEvent| WorkspaceMsg::DragEnd(ev))}
                onmousemove={self.link.callback(|ev: MouseEvent| WorkspaceMsg::Drag(ev))}
            >
                { for self.windows.values().cloned().map(|(ref_, props)|
                    html! { <Window with props ref={ref_} /> }) }
            </div>
        }
    }
}

impl Workspace {
    fn new_window_id(&mut self) -> WindowId {
        let id = self.next_id;
        self.next_id += 1;
        WindowId(id)
    }

    pub fn create_window(&mut self) {
        let id = self.new_window_id();

        let window = WindowProps {
            id: id,
            name: format!("Window #{}", id.0),
            workspace: self.link.clone(),
            position: Coords {
                x: (id.0 as i32 + 1) * 100,
                y: (id.0 as i32 + 1) * 100,
            },
        };

        self.windows.insert(id, (NodeRef::default(), window));
    }
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct WindowId(usize);

pub struct Window {
    link: ComponentLink<Self>,
    props: WindowProps,
}

#[derive(Debug)]
pub enum WindowMsg {
    DragStart(MouseEvent),
}

#[derive(Properties, Clone)]
pub struct WindowProps {
    pub id: WindowId,
    pub name: String,
    pub workspace: ComponentLink<Workspace>,
    pub position: Coords,
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
        crate::log(&format!("{:?}", msg));
        match msg {
            WindowMsg::DragStart(ev) => {
                self.props.workspace.send_message(WorkspaceMsg::DragStart(self.props.id, ev));
            }
        }
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="module-window" style={self.props.position.to_css()}>
                <div class="module-window-title"
                    onmousedown={self.link.callback(|ev: MouseEvent| WindowMsg::DragStart(ev))}
                >
                    {&self.props.name}
                </div>
                <div class="module-window-content">
                </div>
            </div>
        }
    }
}
