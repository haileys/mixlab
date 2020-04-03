use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, GateState};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct GateProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: GateState,
}

pub struct Gate {
    props: GateProps,
}

impl Component for Gate {
    type Properties = GateProps;
    type Message = ();

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Self { props }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        html! {
            <>
                <button
                    onmousedown={self.props.module.callback(move |ev| {
                        WindowMsg::UpdateParams(ModuleParams::Gate(GateState::Open))
                    })}
                    onmouseup={self.props.module.callback(move |ev| {
                        WindowMsg::UpdateParams(ModuleParams::Gate(GateState::Closed))
                    })}
                >{"Trigger"}</button>
            </>
        }
    }
}
