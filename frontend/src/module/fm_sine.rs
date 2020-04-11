use yew::{html, ComponentLink, Html};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, FmSineParams};

use crate::workspace::{Window, WindowMsg};
use crate::component::pure_module::{Pure, PureModule};

pub type FmSine = Pure<FmSineParams>;

impl PureModule for FmSineParams {
    fn view(&self, id: ModuleId, module: ComponentLink<Window>) -> Html {
        let freq_lo_id = format!("w{}-fmsine-freqlo", id.0);
        let freq_hi_id = format!("w{}-fmsine-freqhi", id.0);
        let params = self.clone();

        html! {
            <>
                <label for={&freq_lo_id}>{"Freq Lo"}</label>
                <input type="number"
                    id={&freq_lo_id}
                    onchange={module.callback({
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
                    value={self.freq_lo}
                />

                <label for={&freq_hi_id}>{"Freq Hi"}</label>
                <input type="number"
                    id={&freq_hi_id}
                    onchange={module.callback({
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
                    value={self.freq_hi}
                />
            </>
        }
    }
}
