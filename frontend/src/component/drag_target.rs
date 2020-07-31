use gloo_events::{EventListener, EventListenerOptions};
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef, Callback, Children, Renderable};

pub struct DragTarget {
    link: ComponentLink<Self>,
    props: DragTargetProps,
    container: NodeRef,
    drag_state: Option<DragState>,
}

struct DragState {
    pub origin_x: i32,
    pub origin_y: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub mousemove: EventListener,
    pub mouseup: EventListener,
}

#[derive(Debug)]
pub struct DragEvent {
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Properties, Clone)]
pub struct DragTargetProps {
    #[prop_or_default]
    pub on_drag_start: Option<Callback<DragEvent>>,
    #[prop_or_default]
    pub on_drag: Option<Callback<DragEvent>>,
    #[prop_or_default]
    pub on_drag_end: Option<Callback<DragEvent>>,
    #[prop_or_default]
    pub children: Children,
}

pub enum DragTargetMsg {
    MouseDown(MouseEvent),
    MouseMove(MouseEvent),
    MouseUp(MouseEvent),
}

impl DragTarget {
    fn filter_callback(&self, f: impl Fn(MouseEvent) -> DragTargetMsg + 'static) -> Callback<MouseEvent> {
        let link = self.link.clone();

        Callback::from(move |ev: MouseEvent| {
            if ev.buttons() == 1 {
                ev.stop_propagation();
                link.send_message(f(ev))
            }
        })
    }
}

impl Component for DragTarget {
    type Properties = DragTargetProps;
    type Message = DragTargetMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        DragTarget {
            link,
            props,
            container: NodeRef::default(),
            drag_state: None,
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            DragTargetMsg::MouseDown(ev) => {
                if self.drag_state.is_none() {
                    // register capture-phase mousemove and mouseup events on
                    // window so we can intercept mouse movements even if the
                    // mouse exits our element:

                    let window = web_sys::window().expect("web_sys::window");

                    let mousemove = EventListener::new_with_options(
                        &window, "mousemove", EventListenerOptions::run_in_capture_phase(),
                        {
                            let link = self.link.clone();
                            move |ev| {
                                if let Some(ev) = ev.dyn_ref::<MouseEvent>().cloned() {
                                    ev.stop_propagation();
                                    link.send_message(DragTargetMsg::MouseMove(ev));
                                }
                            }
                        });

                    let mouseup = EventListener::new_with_options(
                        &window, "mouseup", EventListenerOptions::run_in_capture_phase(),
                        {
                            let link = self.link.clone();
                            move |ev| {
                                if let Some(ev) = ev.dyn_ref::<MouseEvent>().cloned() {
                                    ev.stop_propagation();
                                    link.send_message(DragTargetMsg::MouseUp(ev));
                                }
                            }
                        });

                    let offset_x = ev.offset_x();
                    let offset_y = ev.offset_y();

                    self.drag_state = Some(DragState {
                        origin_x: ev.page_x(),
                        origin_y: ev.page_y(),
                        offset_x,
                        offset_y,
                        mousemove,
                        mouseup,
                    });

                    if let Some(callback) = &self.props.on_drag_start {
                        callback.emit(DragEvent {
                            offset_x,
                            offset_y,
                        });
                    }
                }

                false
            }
            DragTargetMsg::MouseMove(ev) => {
                if let Some(drag_state) = &self.drag_state {
                    let page_x = ev.page_x();
                    let page_y = ev.page_y();

                    let offset_x = drag_state.offset_x + (page_x - drag_state.origin_x);
                    let offset_y = drag_state.offset_y + (page_y - drag_state.origin_y);

                    if let Some(callback) = &self.props.on_drag {
                        callback.emit(DragEvent {
                            offset_x,
                            offset_y
                        });
                    }
                }
                false
            }
            DragTargetMsg::MouseUp(ev) => {
                if let Some(drag_state) = self.drag_state.take() {
                    let page_x = ev.page_x();
                    let page_y = ev.page_y();

                    let offset_x = drag_state.offset_x + (page_x - drag_state.origin_x);
                    let offset_y = drag_state.offset_y + (page_y - drag_state.origin_y);

                    if let Some(callback) = &self.props.on_drag_end {
                        callback.emit(DragEvent {
                            offset_x,
                            offset_y
                        });
                    }
                }

                true
            }
        }
    }

    fn view(&self) -> Html {
        html! {
            <div
                class="drag-target-container"
                ref={self.container.clone()}
                onmousedown={self.filter_callback(DragTargetMsg::MouseDown)}
            >
                {self.props.children.clone()}
            </div>
        }
    }
}
