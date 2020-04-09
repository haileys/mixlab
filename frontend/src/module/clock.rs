use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};

use mixlab_protocol::{ModuleId, ModuleParams, ClockParams};

use crate::workspace::{Window, WindowMsg};
use crate::util::extract_callback_float_value;

#[derive(Properties, Clone, Debug)]
pub struct ClockProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: ClockParams,
}

pub struct Clock {
    props: ClockProps,
}

impl Component for Clock {
    type Properties = ClockProps;
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
        let bpm_id = format!("{}-bpm", self.props.id.0);
        let bpm_params = self.props.params.clone();

        html! {
            <>
                <label for={&bpm_id}>{"BPM"}</label>
                <input type="number"
                    id={&bpm_id}
                    min={1}
                    max={512}
                    onchange={self.props.module.callback(move |ev| {
                        let bpm = extract_callback_float_value(ev).unwrap_or(0.0);
                        let params = EnvelopeParams { bpm, ..bpm_params };
                        WindowMsg::UpdateParams(ModuleParams::Envelope(params))
                    })}
                    value={self.props.params.bpm}
            </>
        }
    }
}
