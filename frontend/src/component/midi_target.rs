use std::mem;

use web_sys::MouseEvent;
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, Callback, Children, Renderable};

use crate::service::midi::{self, RangeSubscription, MidiRangeId, ConfigureTask};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MidiUiMode {
    Normal,
    Configure,
}

pub struct MidiRangeTarget {
    link: ComponentLink<Self>,
    props: MidiTargetProps,
    state: MidiState,
}

#[derive(Debug)]
pub enum MidiState {
    Unbound,
    Configure(ConfigureTask),
    Bound(RangeSubscription),
}

#[derive(Properties, Clone)]
pub struct MidiTargetProps {
    pub ui_mode: MidiUiMode,
    pub onchange: Callback<f64>,
    #[prop_or_default]
    pub children: Children,
}

#[derive(Debug)]
pub enum MidiTargetMsg {
    Configure,
    Unbind,
    RangeConfigured(MidiRangeId, u8),
    RangeChanged(u8),
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

    fn change(&mut self, mut props: MidiTargetProps) -> ShouldRender {
        mem::swap(&mut self.props, &mut props);

        if props.ui_mode != self.props.ui_mode {
            match (&self.state, self.props.ui_mode) {
                (MidiState::Configure(_), MidiUiMode::Normal) => {
                    // if we're still in configure state when the UI changes
                    // back to normal mode, return to unbound mode:
                    self.state = MidiState::Unbound;
                }
                _ => { /* otherwise do nothing */ }
            }
        }

        true
    }

    fn update(&mut self, msg: MidiTargetMsg) -> ShouldRender {
        match msg {
            MidiTargetMsg::Configure => {
                let configure = midi::broker().configure_range(self.link.callback(|result| {
                    match result {
                        None => MidiTargetMsg::Unbind,
                        Some((range_id, range_value)) =>
                            MidiTargetMsg::RangeConfigured(range_id, range_value),
                    }
                }));
                self.state = MidiState::Configure(configure);
                true
            }
            MidiTargetMsg::Unbind => {
                self.state = MidiState::Unbound;
                true
            }
            MidiTargetMsg::RangeConfigured(range_id, range_value) => {
                // only handle this message if we're still in configure state:
                if let MidiState::Configure(_) = self.state {
                    let subscription = midi::broker().subscribe_range(range_id,
                        self.link.callback(MidiTargetMsg::RangeChanged));

                    self.props.onchange.emit(range_value as f64 / 127.0);
                    self.state = MidiState::Bound(subscription);
                    true
                } else {
                    false
                }
            }
            MidiTargetMsg::RangeChanged(range_value) => {
                self.props.onchange.emit(range_value as f64 / 127.0);
                false
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
                    MidiState::Configure(_) => "midi-target-overlay midi-target-cfg-overlay midi-target-cfg-overlay-configure",
                    MidiState::Bound(_) => "midi-target-overlay midi-target-cfg-overlay midi-target-cfg-overlay-bound",
                };

                html! {
                    <div
                        class={class}
                        onmousedown={
                            self.link.callback(|ev: MouseEvent| {
                                if ev.buttons() == 2 {
                                    ev.prevent_default();
                                    MidiTargetMsg::Unbind
                                } else {
                                    MidiTargetMsg::Configure
                                }
                            })
                        }
                    ></div>
                }
            }
        };

        html! {
            <div class="midi-target">
                {overlay}
                {self.props.children.clone()}
            </div>
        }
    }
}
