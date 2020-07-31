use std::fmt::{self, Display};

use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::events::ChangeData;
use yew_components::Select;

use mixlab_protocol::{ModuleId, ModuleParams, OscillatorParams, Waveform};

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
        #[derive(PartialEq, Clone)]
        struct SelectableWaveform(Waveform);

        impl Display for SelectableWaveform {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let SelectableWaveform(waveform) = self;
                let name = match waveform {
                    Waveform::Sine => "Sine",
                    Waveform::Square => "Square",
                    Waveform::Saw => "Sawtooth",
                    Waveform::Triangle => "Triangle",
                    Waveform::On => "High",
                    Waveform::Off => "Zero",
                };
                write!(f, "{}", name)
            }
        }

        let waveforms: Vec<SelectableWaveform> = vec![
            SelectableWaveform(Waveform::Sine),
            SelectableWaveform(Waveform::Square),
            SelectableWaveform(Waveform::Saw),
            SelectableWaveform(Waveform::Triangle),
            SelectableWaveform(Waveform::On),
            SelectableWaveform(Waveform::Off),
        ];

        let params = self.props.params.clone();

        html! {
            <>
                <label>
                    <div>{"Waveform"}</div>
                    <Select<SelectableWaveform>
                        selected={SelectableWaveform(params.waveform.clone())}
                        options={waveforms}
                        on_change={self.props.module.callback({
                            let params = self.props.params.clone();
                            move |waveform| {
                                let SelectableWaveform(waveform) = waveform;
                                WindowMsg::UpdateParams(
                                    ModuleParams::Oscillator(OscillatorParams {
                                        waveform,
                                        ..params.clone()
                                    }))
                            }
                        })}
                    />
                </label>
                <label>
                    <div>{"Frequency"}</div>
                    <input type="number"
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
                </label>
            </>
        }
    }
}
