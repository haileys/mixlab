use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, SineGeneratorParams};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct SineGeneratorProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: SineGeneratorParams,
}

pub struct SineGenerator {
    props: SineGeneratorProps,
}

impl Component for SineGenerator {
    type Properties = SineGeneratorProps;
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
        let freq_id = format!("w{}-sine-freq", self.props.id.0);
        let params = self.props.params.clone();

        html! {
            <>
                <label for={&freq_id}>{"Frequency"}</label>
                <input type="number"
                    id={&freq_id}
                    onchange={self.props.module.callback(move |ev| {
                        if let ChangeData::Value(freq_str) = ev {
                            let freq = freq_str.parse().unwrap_or(0.0);
                            let params = SineGeneratorParams { freq, ..params };
                            WindowMsg::UpdateParams(
                                ModuleParams::SineGenerator(params))
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
