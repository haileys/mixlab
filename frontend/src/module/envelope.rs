use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, EnvelopeParams};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct EnvelopeProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: EnvelopeParams,
}

pub struct Envelope {
    props: EnvelopeProps,
}

impl Component for Envelope {
    type Properties = EnvelopeProps;
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
        let attack_id = format!("w{}-attack", self.props.id.0);
        let attack_params = self.props.params.clone();

        let decay_id = format!("w{}-decay", self.props.id.0);
        let decay_params = self.props.params.clone();

        let sustain_id = format!("w{}-sustain", self.props.id.0);
        let sustain_params = self.props.params.clone();

        let release_id = format!("w{}-release", self.props.id.0);
        let release_params = self.props.params.clone();

        html! {
            <>
                <label for={&attack_id}>{"Attack"}</label>
                <input type="range"
                    id={&attack_id}
                    min={0}
                    max={500}
                    step={1}
                    onchange={self.props.module.callback(move |ev| {
                        let attack_ms = extract_float_value(ev).unwrap_or(0.0);
                        let params = EnvelopeParams { attack_ms, ..attack_params };
                        WindowMsg::UpdateParams(ModuleParams::Envelope(params))
                    })}
                    value={self.props.params.attack_ms}
                />
                <label for={&decay_id}>{"Decay"}</label>
                <input type="range"
                    id={&decay_id}
                    min={5}
                    max={1000}
                    step={1}
                    onchange={self.props.module.callback(move |ev| {
                        let decay_ms = extract_float_value(ev).unwrap_or(0.0);
                        let params = EnvelopeParams { decay_ms, ..decay_params };
                        WindowMsg::UpdateParams(ModuleParams::Envelope(params))
                    })}
                    value={self.props.params.decay_ms}
                />
                <label for={&sustain_id}>{"Sustain"}</label>
                <input type="range"
                    id={&sustain_id}
                    min={0}
                    max={1}
                    step={0.01}
                    onchange={self.props.module.callback(move |ev| {
                        let sustain_amplitude = extract_float_value(ev).unwrap_or(0.0);
                        let params = EnvelopeParams { sustain_amplitude, ..sustain_params };
                        WindowMsg::UpdateParams(ModuleParams::Envelope(params))
                    })}
                    value={self.props.params.sustain_amplitude}
                />
                <label for={&release_id}>{"Release"}</label>
                <input type="range"
                    id={&release_id}
                    min={0}
                    max={5000}
                    step={1}
                    onchange={self.props.module.callback(move |ev| {
                        let release_ms = extract_float_value(ev).unwrap_or(0.0);
                        let params = EnvelopeParams { release_ms, ..release_params };
                        WindowMsg::UpdateParams(ModuleParams::Envelope(params))
                    })}
                    value={self.props.params.release_ms}
                />
            </>
        }
    }
}

fn extract_float_value(event: ChangeData) -> Option<f32> {
    match event {
        ChangeData::Value(float_str) => float_str.parse().ok(),
        _ => None
    }
}
