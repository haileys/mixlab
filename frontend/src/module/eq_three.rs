use yew::{html, ComponentLink, Html};

use mixlab_protocol::{ModuleId, ModuleParams, EqThreeParams, Decibel};

use crate::component::midi_target::{MidiRangeTarget, MidiUiMode};
use crate::component::pure_module::{Pure, PureModule};
use crate::control::rotary::Rotary;
use crate::workspace::{Window, WindowMsg};

pub type EqThree = Pure<EqThreeParams>;

impl PureModule for EqThreeParams {
    fn view(&self, _: ModuleId, module: ComponentLink<Window>, midi_mode: MidiUiMode) -> Html {

        html! {
            <>
                <div>{"HI"}</div>
                <MidiRangeTarget
                    ui_mode={midi_mode}
                    onchange={module.callback(wrap_decibel(update_params(self,
                        |params, value| EqThreeParams { gain_hi: value, ..params })))}
                >
                    <Rotary<Decibel>
                        value={self.gain_hi}
                        min={Decibel(-24.0)}
                        max={Decibel(6.0)}
                        default={Decibel(0.0)}
                        onchange={module.callback(update_params(self,
                            |params, value| EqThreeParams { gain_hi: value, ..params }))}
                    />
                </MidiRangeTarget>

                <div>{"MID"}</div>
                <MidiRangeTarget
                    ui_mode={midi_mode}
                    onchange={module.callback(wrap_decibel(update_params(self,
                        |params, value| EqThreeParams { gain_mid: value, ..params })))}
                >
                    <Rotary<Decibel>
                        value={self.gain_mid}
                        min={Decibel(-24.0)}
                        max={Decibel(6.0)}
                        default={Decibel(0.0)}
                        onchange={module.callback(update_params(self,
                            |params, value| EqThreeParams { gain_mid: value, ..params }))}
                    />
                </MidiRangeTarget>

                <div>{"LO"}</div>
                <MidiRangeTarget
                    ui_mode={midi_mode}
                    onchange={module.callback(wrap_decibel(update_params(self,
                        |params, value| EqThreeParams { gain_lo: value, ..params })))}
                >
                    <Rotary<Decibel>
                        value={self.gain_lo}
                        min={Decibel(-24.0)}
                        max={Decibel(6.0)}
                        default={Decibel(0.0)}
                        onchange={module.callback(update_params(self,
                            |params, value| EqThreeParams { gain_lo: value, ..params }))}
                    />
                </MidiRangeTarget>
            </>
        }
    }
}

fn update_params(params: &EqThreeParams, f: impl Fn(EqThreeParams, Decibel) -> EqThreeParams) -> impl Fn(Decibel) -> WindowMsg {
    let params = params.clone();
    move |value: Decibel| {
        let params = f(params.clone(), value);
        WindowMsg::UpdateParams(ModuleParams::EqThree(params))
    }
}

fn wrap_decibel<'a, T>(f: impl Fn(Decibel) -> T + 'a) -> impl Fn(f64) -> T + 'a {
    move |gain| f(Decibel(gain * 30.0 - 24.0))
}
