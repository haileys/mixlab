use web_sys::MouseEvent;
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, Callback, Children, Renderable};

use crate::service::midi::{self, RangeSubscription};

#[derive(Clone, Copy)]
pub enum MidiUiMode {
    Normal,
    Configure,
}

pub struct MidiRangeTarget {
    link: ComponentLink<Self>,
    props: MidiTargetProps,
    state: MidiState,
}

pub enum MidiState {
    Unbound,
    Configure,
    Bound(RangeSubscription),
}

#[derive(Properties, Clone)]
pub struct MidiTargetProps {
    pub ui_mode: MidiUiMode,
    pub onchange: Callback<f64>,
    #[prop_or_default]
    pub children: Children,
}

pub enum MidiTargetMsg {
    Configure,
    Unbind,
    Bind(RangeSubscription),
}

impl Component for MidiRangeTarget {
    type Properties = MidiTargetProps;
    type Message = MidiTargetMsg;

    fn create(props: MidiTargetProps, link: ComponentLink<Self>) -> Self {
        MidiRangeTarget {
            props,
            link,
            state: MidiState::Unbound,
        }
    }

    fn change(&mut self, props: MidiTargetProps) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: MidiTargetMsg) -> ShouldRender {
        match msg {
            MidiTargetMsg::Configure => {
                self.state = MidiState::Configure;

                midi::broker().configure_range({
                    let link = self.link.clone();
                    let onchange = self.props.onchange.clone();
                    Callback::from(move |result| {
                        match result {
                            Some((range_id, value)) => {
                                onchange.emit(value as f64 / 127.0);

                                let subscription = midi::broker().subscribe_range(range_id, {
                                    let onchange = onchange.clone();
                                    Callback::from(move |value| {
                                        onchange.emit(value as f64 / 127.0);
                                    })
                                });

                                link.send_message(MidiTargetMsg::Bind(subscription));
                            }
                            None => {
                                link.send_message(MidiTargetMsg::Unbind);
                            }
                        }
                    })
                });

                true
            }
            MidiTargetMsg::Bind(subscription) => {
                self.state = MidiState::Bound(subscription);
                true
            }
            MidiTargetMsg::Unbind => {
                self.state = MidiState::Unbound;
                true
            }
        }
    }

    fn view(&self) -> Html {
        let overlay = match self.props.ui_mode {
            MidiUiMode::Normal => {
                if let MidiState::Bound(_) = self.state {
                    html! {
                        <div class="midi-target-overlay midi-target-overlay-bound">
                            <span class="midi-target-overlay-label">{"MIDI"}</span>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
            MidiUiMode::Configure => {
                let class = match self.state {
                    MidiState::Unbound => "midi-target-overlay midi-target-cfg-overlay midi-target-cfg-overlay-unbound",
                    MidiState::Configure => "midi-target-overlay midi-target-cfg-overlay midi-target-cfg-overlay-configure",
                    MidiState::Bound(_) => "midi-target-overlay midi-target-cfg-overlay midi-target-cfg-overlay-bound",
                };

                html! {
                    <div class={class} onmousedown={self.overlay_mousedown()}></div>
                }
            }
        };

        html! {
            <div class="midi-target">
                {overlay}
                {self.props.children.render()}
            </div>
        }
    }
}

impl MidiRangeTarget {
    fn overlay_mousedown(&self) -> Callback<MouseEvent> {
        let link = self.link.clone();

        Callback::from(move |ev: MouseEvent| {
            ev.stop_propagation();

            if ev.buttons() == 2 {
                ev.prevent_default();
                link.send_message(MidiTargetMsg::Unbind);
            } else {
                link.send_message(MidiTargetMsg::Configure);
            }
        })
    }
}
