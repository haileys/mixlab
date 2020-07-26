use yew::{html, ComponentLink, Html, Callback};

use mixlab_protocol::{ModuleId, ModuleParams, VideoMixerParams, VIDEO_MIXER_CHANNELS};

use crate::component::pure_module::{Pure, PureModule};
use crate::component::midi_target::{MidiRangeTarget, MidiUiMode};
use crate::control::Fader;
use crate::workspace::{Window, WindowMsg};

pub type VideoMixer = Pure<VideoMixerParams>;

impl PureModule for VideoMixerParams {
    fn view(&self, _: ModuleId, module: ComponentLink<Window>, midi_mode: MidiUiMode) -> Html {
        html! {
            <>
                <div class="video-mixer">
                    <div class="video-mixer-channels">
                        <div class="video-mixer-channel-row">
                            {view_channel_row(Selector::A, self.a, module.callback(
                                update_params(self, move |params, selection|
                                    VideoMixerParams { a: selection, ..params })))}
                        </div>

                        <div class="video-mixer-channel-row">
                            {view_channel_row(Selector::B, self.b, module.callback(
                                update_params(self, move |params, selection|
                                    VideoMixerParams { b: selection, ..params })))}
                        </div>
                    </div>
                    <div class="video-mixer-fader">
                        <MidiRangeTarget
                            ui_mode={midi_mode}
                            onchange={module.callback(
                                update_params(self, move |params, fader|
                                    VideoMixerParams { fader, ..params }))}
                        >
                            <Fader
                                value={self.fader}
                                onchange={module.callback(
                                    update_params(self, move |params, fader|
                                        VideoMixerParams { fader, ..params }))}
                            />
                        </MidiRangeTarget>
                    </div>
                </div>
            </>
        }
    }
}

enum Selector {
    A,
    B,
}

fn view_channel_row(sel: Selector, current: Option<usize>, onchange: Callback<Option<usize>>) -> Html {
    html! {
        {for (0..VIDEO_MIXER_CHANNELS).map(|i| {
            let class = if Some(i) == current {
                match sel {
                    Selector::A => "video-mixer-channel-select-btn video-mixer-channel-selected-a",
                    Selector::B => "video-mixer-channel-select-btn video-mixer-channel-selected-b",
                }
            } else {
                "video-mixer-channel-select-btn"
            };

            html! {
                <button
                    class={class}
                    onclick={onchange.reform(move |_| Some(i))}
                >
                    {(i + 1).to_string()}
                </button>
            }
        })}
    }
}

fn update_params<T>(params: &VideoMixerParams, f: impl Fn(VideoMixerParams, T) -> VideoMixerParams) -> impl Fn(T) -> WindowMsg {
    let params = params.clone();
    move |arg| WindowMsg::UpdateParams(ModuleParams::VideoMixer(f(params.clone(), arg)))
}
