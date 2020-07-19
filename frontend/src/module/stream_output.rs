use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, Callback};
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, StreamOutputParams, StreamOutputLiveStatus, StreamOutputIndication};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct StreamOutputProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: StreamOutputParams,
    pub indication: StreamOutputIndication,
}

pub struct StreamOutput {
    props: StreamOutputProps,
}

impl Component for StreamOutput {
    type Properties = StreamOutputProps;
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
        let is_conn_active = match self.props.indication.live {
            StreamOutputLiveStatus::Offline => false,
            StreamOutputLiveStatus::Connecting | StreamOutputLiveStatus::Live => true,
        };

        html! {
            <>
                <div class="status-light-bar">
                    <div class={live_class(self.props.indication.live)}>{"LIVE"}</div>
                    <div class={warning_class(self.props.indication.error)}>{"ERROR"}</div>
                </div>

                { if is_conn_active {
                    html! {
                        <button
                            onclick={self.callback(move |_, params| {
                                StreamOutputParams { disconnect_seq: params.seq, ..params }
                            })}
                        >
                            {"Disconnect"}
                        </button>
                    }
                } else {
                    html! {
                        <button
                            onclick={self.callback(move |_, params| {
                                StreamOutputParams { connect_seq: params.seq, ..params }
                            })}
                        >
                            {"Connect"}
                        </button>
                    }
                } }

                <label class="form-field">
                    <span class="form-field-label">{"RTMP URL"}</span>
                    <input type="text"
                        onchange={self.callback(text(move |rtmp_url, params| {
                            StreamOutputParams { rtmp_url, ..params }
                        }))}
                        value={&self.props.params.rtmp_url}
                    />
                </label>

                <label class="form-field">
                    <span class="form-field-label">{"Stream Key"}</span>
                    <input type="text"
                        onchange={self.callback(text(move |rtmp_stream_key, params| {
                            StreamOutputParams { rtmp_stream_key, ..params }
                        }))}
                        value={&self.props.params.rtmp_stream_key}
                    />
                </label>
            </>
        }
    }
}

impl StreamOutput {
    fn callback<Ev>(&self, f: impl Fn(Ev, StreamOutputParams) -> StreamOutputParams + 'static)
        -> Callback<Ev>
    {
        let params = self.props.params.clone();

        self.props.module.callback(move |ev| {
            let updated_params = f(ev, {
                let mut params = params.clone();
                params.seq += 1;
                params
            });

            WindowMsg::UpdateParams(
                ModuleParams::StreamOutput(updated_params))
        })
    }
}

fn text<T>(f: impl Fn(String, StreamOutputParams) -> T)
    -> impl Fn(ChangeData, StreamOutputParams) -> T
{
    move |change, params| {
        if let ChangeData::Value(value) = change {
            f(value, params)
        } else {
            unreachable!()
        }
    }
}

fn live_class(live_status: StreamOutputLiveStatus) -> &'static str {
    match live_status {
        StreamOutputLiveStatus::Offline => "status-light",
        StreamOutputLiveStatus::Connecting => "status-light status-light-green",
        StreamOutputLiveStatus::Live => "status-light status-light-green-active",
    }
}

fn warning_class(is_warning: bool) -> &'static str {
    match is_warning {
        false => "status-light",
        true => "status-light status-light-red-active",
    }
}
