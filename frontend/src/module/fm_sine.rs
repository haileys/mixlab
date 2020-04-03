use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, FmSineParams};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct FmSineProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: FmSineParams,
}

pub struct FmSine {
    props: FmSineProps,
}

impl Component for FmSine {
    type Properties = FmSineProps;
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
        let freq_lo_id = format!("w{}-fmsine-freqlo", self.props.id.0);
        let freq_hi_id = format!("w{}-fmsine-freqhi", self.props.id.0);
        let params = self.props.params.clone();

        html! {
            <>
                <label for={&freq_lo_id}>{"Freq Lo"}</label>
                <input type="number"
                    id={&freq_lo_id}
                    onchange={self.props.module.callback({
                        let params = params.clone();
                        move |ev| {
                            if let ChangeData::Value(freq_str) = ev {
                                let freq_lo = freq_str.parse().unwrap_or(0.0);
                                let params = FmSineParams { freq_lo, ..params };
                                WindowMsg::UpdateParams(
                                    ModuleParams::FmSine(params))
                            } else {
                                unreachable!()
                            }
                        }
                    })}
                    value={self.props.params.freq_lo}
                />

                <label for={&freq_hi_id}>{"Freq Hi"}</label>
                <input type="number"
                    id={&freq_hi_id}
                    onchange={self.props.module.callback({
                        let params = params.clone();
                        move |ev| {
                            if let ChangeData::Value(freq_str) = ev {
                                let freq_hi = freq_str.parse().unwrap_or(0.0);
                                let params = FmSineParams { freq_hi, ..params };
                                WindowMsg::UpdateParams(
                                    ModuleParams::FmSine(params))
                            } else {
                                unreachable!()
                            }
                        }
                    })}
                    value={self.props.params.freq_hi}
                />
            </>
        }
    }
}
