use std::fmt::{self, Display};
use std::iter;

use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew::components::Select;

use mixlab_protocol::{ModuleId, ModuleParams, OutputDeviceParams, OutputDeviceIndication, OutputDeviceWarning};

use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct OutputDeviceProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: OutputDeviceParams,
    pub indication: OutputDeviceIndication,
}

pub struct OutputDevice {
    props: OutputDeviceProps,
}

impl Component for OutputDevice {
    type Properties = OutputDeviceProps;
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
        struct OutputChannel(Option<usize>);

        impl Display for OutputChannel {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.0 {
                    Some(ch) => {
                        // channels are 0-indexed internally, but 1-indexed in the UI:
                        let display_channel_number = ch + 1;

                        write!(f, "Channel #{}", display_channel_number)
                    }
                    None => {
                        write!(f, "None")
                    }
                }
            }
        }

        let devices = self.props.indication.devices.as_ref()
            .map(|devices| devices.as_slice())
            .unwrap_or(&[]);

        let device_names = devices.iter()
            .map(|(device_name, _)| device_name)
            .cloned()
            .collect::<Vec<_>>();

        let channels = iter::once(None)
            .chain(
                devices.iter()
                    .find(|(dev, _)| Some(dev) == self.props.params.device.as_ref())
                    .into_iter()
                    .flat_map(|(_, channel_count)| 0..*channel_count)
                    .map(Some))
            .map(OutputChannel)
            .collect::<Vec<_>>();

        html! {
            <>
                <div class="status-light-bar">
                    <div class={warning_class(self.props.indication.clip)}>{"CLIP"}</div>
                    <div class={warning_class(self.props.indication.lag)}>{"LAG"}</div>
                </div>
                <button
                    onclick={self.props.module.callback({
                        let device = self.props.indication.default_device.clone();

                        let channel_count = devices.iter()
                            .find(|(name, _)| Some(name) == device.as_ref())
                            .map(|(_, channels)| channels);

                        let left = Some(0).filter(|ch| channel_count >= Some(ch));
                        let right = Some(1).filter(|ch| channel_count >= Some(ch));

                        let params = OutputDeviceParams {
                            device,
                            left,
                            right,
                            ..self.props.params.clone()
                        };

                        move |_| WindowMsg::UpdateParams(
                            ModuleParams::OutputDevice(params.clone()))
                    })}
                >
                    {"Use system defaults"}
                </button>

                <label>{"Output device"}</label>
                <Select<String>
                    selected={&self.props.params.device}
                    options={device_names}
                    onchange={self.props.module.callback({
                        let params = self.props.params.clone();
                        move |device: String| {
                            let params = OutputDeviceParams { device: Some(device), ..params.clone() };
                            WindowMsg::UpdateParams(ModuleParams::OutputDevice(params))
                        }
                    })}
                />

                <label>{"Left channel"}</label>
                <Select<OutputChannel>
                    selected={OutputChannel(self.props.params.left)}
                    options={channels.clone()}
                    onchange={self.props.module.callback({
                        let params = self.props.params.clone();
                        move |chan: OutputChannel| {
                            let params = OutputDeviceParams { left: chan.0, ..params.clone() };
                            WindowMsg::UpdateParams(ModuleParams::OutputDevice(params))
                        }
                    })}
                />

                <label>{"Right channel"}</label>
                <Select<OutputChannel>
                    selected={OutputChannel(self.props.params.right)}
                    options={channels}
                    onchange={self.props.module.callback({
                        let params = self.props.params.clone();
                        move |chan: OutputChannel| {
                            let params = OutputDeviceParams { right: chan.0, ..params.clone() };
                            WindowMsg::UpdateParams(ModuleParams::OutputDevice(params))
                        }
                    })}
                />
            </>
        }
    }
}

fn warning_class(warning_status: Option<OutputDeviceWarning>) -> &'static str {
    match warning_status {
        None => "status-light",
        Some(OutputDeviceWarning::Active) => "status-light status-light-red-active",
        Some(OutputDeviceWarning::Recent) => "status-light status-light-red",
    }
}
