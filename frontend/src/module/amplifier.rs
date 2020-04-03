use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, AmplifierParams};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct AmplifierProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: AmplifierParams,
}

pub struct Amplifier {
    props: AmplifierProps,
}

impl Component for Amplifier {
    type Properties = AmplifierProps;
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
        let amp_id = format!("w{}-amp", self.props.id.0);
        let amp_params = self.props.params.clone();

        let mod_id = format!("w{}-mod", self.props.id.0);
        let mod_params = self.props.params.clone();

        html! {
            <>
                <label for={&amp_id}>{"Volume"}</label>
                <input type="range"
                    id={&amp_id}
                    min={0}
                    max={1}
                    step={0.01}
                    onchange={self.props.module.callback(move |ev| {
                        if let ChangeData::Value(amplitude_str) = ev {
                            let amplitude = amplitude_str.parse().unwrap_or(0.0);
                            let params = AmplifierParams { amplitude, ..amp_params };
                            WindowMsg::UpdateParams(
                                ModuleParams::Amplifier(params))
                        } else {
                            unreachable!()
                        }
                    })}
                    value={self.props.params.amplitude}
                />
                <label for={&mod_id}>{"Mod Depth"}</label>
                <input type="range"
                    id={&mod_id}
                    min={0}
                    max={1}
                    step={0.01}
                    onchange={self.props.module.callback(move |ev| {
                        if let ChangeData::Value(mod_str) = ev {
                            let mod_depth = mod_str.parse().unwrap_or(0.0);
                            let params = AmplifierParams { mod_depth, ..mod_params };
                            WindowMsg::UpdateParams(
                                ModuleParams::Amplifier(params))
                        } else {
                            unreachable!()
                        }
                    })}
                    value={self.props.params.mod_depth}
                />
            </>
        }
    }
}
