use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, Callback};

use mixlab_protocol::{ModuleId, MixerParams, MixerChannelParams, ModuleParams, Decibel};

use crate::component::midi_target::MidiRangeTarget;
use crate::control::{Fader, Rotary};
use crate::workspace::{Window, WindowMsg};

pub struct Mixer {
    link: ComponentLink<Self>,
    props: MixerProps,
}

#[derive(Properties, Clone)]
pub struct MixerProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: MixerParams,
}

pub enum MixerMsg {
    ChannelChanged(usize, MixerChannelParams),
}

impl Component for Mixer {
    type Properties = MixerProps;
    type Message = MixerMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Mixer { link, props }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            MixerMsg::ChannelChanged(idx, chan) => {
                let mut params = self.props.params.clone();
                params.channels[idx] = chan;
                self.props.module.send_message(
                    WindowMsg::UpdateParams(
                        ModuleParams::Mixer(params)));
                false
            }
        }
    }

    fn view(&self) -> Html {
        html! {
            <div class="mixer-channels">
                { for self.props.params.channels.iter()
                    .enumerate()
                    .map(|(idx, channel)| {
                        html! {
                            <Channel
                                params={channel}
                                onchange={self.link.callback(move |params|
                                    MixerMsg::ChannelChanged(idx, params))}
                            />
                        }
                    })
                }
            </div>
        }
    }
}

pub struct Channel {
    link: ComponentLink<Self>,
    props: ChannelProps,
}

pub enum ChannelMsg {
    GainChanged(Decibel),
    CueClick,
    FaderChanged(f64),
}

#[derive(Properties, Clone)]
pub struct ChannelProps {
    pub params: MixerChannelParams,
    pub onchange: Callback<MixerChannelParams>,
}

impl Component for Channel {
    type Properties = ChannelProps;
    type Message = ChannelMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Channel { link, props }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        let params = self.props.params.clone();

        match msg {
            ChannelMsg::GainChanged(gain) => {
                self.props.onchange.emit(MixerChannelParams {
                    gain,
                    ..params
                })
            }
            ChannelMsg::CueClick => {
                self.props.onchange.emit(MixerChannelParams {
                    cue: !params.cue,
                    ..params
                });
            }
            ChannelMsg::FaderChanged(value) => {
                self.props.onchange.emit(MixerChannelParams {
                    fader: value,
                    ..params
                });
            }
        }

        false
    }

    fn view(&self) -> Html {
        let cue_style = if self.props.params.cue {
            "mixer-channel-cue-btn mixer-channel-cue-on"
        } else {
            "mixer-channel-cue-btn"
        };

        html! {
            <div class="mixer-channel">
                <MidiRangeTarget
                    onchange={self.link.callback(|gain| {
                        ChannelMsg::GainChanged(Decibel(gain * 30.0 - 24.0))
                    })}
                >
                    <Rotary<Decibel>
                        value={self.props.params.gain}
                        min={Decibel(-24.0)}
                        max={Decibel(6.0)}
                        default={Decibel(0.0)}
                        onchange={self.link.callback(ChannelMsg::GainChanged)}
                    />
                </MidiRangeTarget>
                <div class={cue_style} onclick={self.link.callback(|_| ChannelMsg::CueClick)}>
                    {"CUE"}
                </div>
                <MidiRangeTarget onchange={self.link.callback(ChannelMsg::FaderChanged)}>
                    <Fader
                        value={self.props.params.fader}
                        onchange={self.link.callback(ChannelMsg::FaderChanged)}
                    />
                </MidiRangeTarget>
            </div>
        }
    }
}
