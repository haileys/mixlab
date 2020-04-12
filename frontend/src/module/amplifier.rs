use yew::{html, ComponentLink, Html};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, AmplifierParams};

use crate::workspace::{Window, WindowMsg};
use crate::component::pure_module::{Pure, PureModule};

pub type Amplifier = Pure<AmplifierParams>;

impl PureModule for AmplifierParams {
    fn view(&self, id: ModuleId, module: ComponentLink<Window>) -> Html {
        let amp_id = format!("w{}-amp", id.0);
        let amp_params = self.clone();

        let mod_id = format!("w{}-mod", id.0);
        let mod_params = self.clone();

        html! {
            <>
                <label for={&amp_id}>{"Volume"}</label>
                <input type="range"
                    id={&amp_id}
                    min={0}
                    max={1}
                    step={0.01}
                    onchange={module.callback(move |ev| {
                        if let ChangeData::Value(amplitude_str) = ev {
                            let amplitude = amplitude_str.parse().unwrap_or(0.0);
                            let params = AmplifierParams { amplitude, ..amp_params };
                            WindowMsg::UpdateParams(
                                ModuleParams::Amplifier(params))
                        } else {
                            unreachable!()
                        }
                    })}
                    value={self.amplitude}
                />
                <label for={&mod_id}>{"Mod Depth"}</label>
                <input type="range"
                    id={&mod_id}
                    min={0}
                    max={1}
                    step={0.01}
                    onchange={module.callback(move |ev| {
                        if let ChangeData::Value(mod_str) = ev {
                            let mod_depth = mod_str.parse().unwrap_or(0.0);
                            let params = AmplifierParams { mod_depth, ..mod_params };
                            WindowMsg::UpdateParams(
                                ModuleParams::Amplifier(params))
                        } else {
                            unreachable!()
                        }
                    })}
                    value={self.mod_depth}
                />
            </>
        }
    }
}
