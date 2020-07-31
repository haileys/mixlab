use std::fmt::{self, Display};

use derive_more::{From, Into};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, Callback};
use yew_components::Select;
use yew::events::ChangeData;

use mixlab_protocol::{ModuleId, ModuleParams, StreamInputParams, StreamProtocol};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct StreamInputProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: StreamInputParams,
}

pub struct StreamInput {
    props: StreamInputProps,
}

impl Component for StreamInput {
    type Properties = StreamInputProps;
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
        html! {
            <>
                <label class="form-field">
                    <span class="form-field-label">{"Protocol"}</span>
                    <Select<DisplayProtocol>
                        selected={self.props.params.protocol.map(DisplayProtocol)}
                        options={vec![
                            DisplayProtocol(StreamProtocol::Icecast),
                            DisplayProtocol(StreamProtocol::Rtmp),
                        ]}
                        on_change={self.callback(move |protocol: DisplayProtocol, params| {
                            StreamInputParams { protocol: Some(protocol.0), ..params }
                        })}
                    />
                </label>

                <label class="form-field">
                    <span class="form-field-label">{"Mountpoint"}</span>
                    <input type="text"
                        onchange={self.callback(text(move |mountpoint, params| {
                            StreamInputParams {
                                mountpoint: mountpoint.map(str::to_owned),
                                ..params
                            }
                        }))}
                        value={self.props.params.mountpoint.as_ref().map(String::as_str).unwrap_or("")}
                    />
                </label>
            </>
        }
    }
}

impl StreamInput {
    fn callback<Ev>(&self, f: impl Fn(Ev, StreamInputParams) -> StreamInputParams + 'static)
        -> Callback<Ev>
    {
        let params = self.props.params.clone();

        self.props.module.callback(move |ev|
            WindowMsg::UpdateParams(
                ModuleParams::StreamInput(
                    f(ev, params.clone()))))
    }
}

fn text<T>(f: impl Fn(Option<&str>, StreamInputParams) -> T)
    -> impl Fn(ChangeData, StreamInputParams) -> T
{
    move |change, params| {
        if let ChangeData::Value(value) = change {
            let str_value = match value.as_str() {
                "" => None,
                s => Some(s),
            };

            f(str_value, params)
        } else {
            unreachable!()
        }
    }
}

#[derive(From, Into, PartialEq, Clone)]
pub struct DisplayProtocol(StreamProtocol);

impl Display for DisplayProtocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            StreamProtocol::Icecast => write!(f, "Icecast"),
            StreamProtocol::Rtmp => write!(f, "RTMP"),
        }
    }
}
