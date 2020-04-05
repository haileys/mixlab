use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, Callback};

use mixlab_protocol::{ModuleId, Mixer4chParams, MixerChannelParams, ModuleParams, Decibel};

use crate::control::{Fader, Rotary};
use crate::workspace::{Window, WindowMsg};

pub struct Mixer4ch {
    link: ComponentLink<Self>,
    props: Mixer4chProps,
}

#[derive(Properties, Clone)]
pub struct Mixer4chProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: Mixer4chParams,
}

pub enum Mixer4chMsg {
    ChannelChanged(usize, MixerChannelParams),
}

impl Component for Mixer4ch {
    type Properties = Mixer4chProps;
    type Message = Mixer4chMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Mixer4ch { link, props }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Mixer4chMsg::ChannelChanged(idx, chan) => {
                let mut params = self.props.params.clone();
                params.channels[idx] = chan;
                self.props.module.send_message(
                    WindowMsg::UpdateParams(
                        ModuleParams::Mixer4ch(params)));
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
                                    Mixer4chMsg::ChannelChanged(idx, params))}
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
                <Rotary<Decibel>
                    value={self.props.params.gain}
                    min={Decibel(-24.0)}
                    max={Decibel(6.0)}
                    default={Decibel(0.0)}
                    onchange={self.link.callback(ChannelMsg::GainChanged)}
                />
                <div class={cue_style} onclick={self.link.callback(|_| ChannelMsg::CueClick)}>
                    {"CUE"}
                </div>
                <Fader
                    value={self.props.params.fader}
                    onchange={self.link.callback(ChannelMsg::FaderChanged)}
                />
            </div>
        }
    }
}
