use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

use mixlab_protocol::{ModuleId};

#[derive(Properties, Clone, Debug)]
pub struct MonitorProps {
    pub id: ModuleId,
}

pub struct Monitor {
    props: MonitorProps,
    video_element: NodeRef,
}

impl Component for Monitor {
    type Properties = MonitorProps;
    type Message = ();

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Monitor {
            props,
            video_element: NodeRef::default(),
        }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn mounted(&mut self) -> ShouldRender {
        true
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        html! {
            <video width={400} height={250} ref={self.video_element.clone()} />
        }
    }
}
