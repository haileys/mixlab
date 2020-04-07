use gloo_events::{EventListener, EventListenerOptions};
use wasm_bindgen::JsCast;
use web_sys::{WheelEvent, Element};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef, Children, Callback, Renderable};

#[derive(Properties, Clone)]
pub struct ScrollProps {
    #[prop_or_default]
    pub on_scroll: Option<Callback<Scroll>>,

    #[prop_or_default]
    pub children: Children,
}

#[derive(Debug, Clone)]
pub enum Scroll {
    Up(f64),
    Down(f64),
}

pub struct ScrollTarget {
    link: ComponentLink<Self>,
    props: ScrollProps,
    container: NodeRef,
    // state: Option<ScrollState>,
}

impl Component for ScrollTarget {
    type Properties = ScrollProps;
    type Message = ();

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        ScrollTarget {
            link,
            props,
            container: NodeRef::default(),
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        crate::log!("{:?}", msg);
        false
    }

    // yew doesn't know about `onwheel` and tries to call `to_string()`, so
    // attaching event handler manually.
    fn mounted(&mut self) -> ShouldRender {
        if let Some(el) = self.container.cast::<Element>() {
            let options = EventListenerOptions::enable_prevent_default();
            let wheel = EventListener::new_with_options(
                &el, "wheel", options,
                {
                    let on_scroll = self.props.on_scroll.clone();
                    move |ev| {
                        if let Some(on_scroll) = &on_scroll {
                            if let Some(ev) = ev.dyn_ref::<WheelEvent>().cloned() {
                                let delta = ev.delta_y();

                                let scroll = if delta < 0.0 {
                                    Scroll::Up(delta.abs())
                                } else {
                                    Scroll::Down(delta.abs())
                                };

                                ev.prevent_default();
                                ev.stop_propagation();
                                on_scroll.emit(scroll);
                            }
                        }
                    }
                }
            );

            // TODO: maybe store this in the state. Unclear when this is dropped
            // if forgotten.
            wheel.forget();
        }

        false
    }

    fn view(&self) -> Html {
        html! {
            <div
                class="scroll-target-container"
                ref={self.container.clone()}
            >
                {self.props.children.render()}
            </div>
        }
    }
}
