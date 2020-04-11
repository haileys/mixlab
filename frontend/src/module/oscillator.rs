use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, OscillatorParams};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct OscillatorProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: OscillatorParams,
}

pub struct Oscillator {
    props: OscillatorProps,
}

impl Component for Oscillator {
    type Properties = OscillatorProps;
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
        let freq_id = format!("w{}-oscillator-freq", self.props.id.0);
        let params = self.props.params.clone();

        html! {
            <>
                <label for={&freq_id}>{"Frequency"}</label>
                <input type="number"
                    id={&freq_id}
                    onchange={self.props.module.callback(move |ev| {
                        if let ChangeData::Value(freq_str) = ev {
                            let freq = freq_str.parse().unwrap_or(0.0);
                            let params = OscillatorParams { freq, ..params };
                            WindowMsg::UpdateParams(
                                ModuleParams::Oscillator(params))
                        } else {
                            unreachable!()
                        }
                    })}
                    value={self.props.params.freq}
                />
            </>
        }
    }
}
