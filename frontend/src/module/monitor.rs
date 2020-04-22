use std::mem;

use gloo_events::EventListener;
use web_sys::{MediaSource, SourceBuffer, Url, HtmlVideoElement};
use yew::format::Binary;
use yew::services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef, Callback};

use mixlab_protocol::{ModuleId, MonitorIndication};

#[derive(Properties, Clone, Debug)]
pub struct MonitorProps {
    pub id: ModuleId,
    pub indication: MonitorIndication,
}

pub struct Monitor {
    link: ComponentLink<Self>,
    props: MonitorProps,
    socket: WebSocketTask,
    media_source: MediaSource,
    source_buffer: Option<SourceBuffer>,
    fragment: Vec<u8>,
    received: usize,
    ready: bool,
    source_url: String,
    video_element: NodeRef,
    _source_open_event: EventListener,
    _buffer_ready_event: Option<EventListener>,
}

pub enum MonitorMsg {
    SourceOpen,
    SourceBufferUpdate,
    FragmentReceive(Vec<u8>),
}

impl Component for Monitor {
    type Properties = MonitorProps;
    type Message = MonitorMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut socket = WebSocketService::new();
        let socket_id = format!("ws://localhost:8000/_monitor/{}", props.indication.socket_id);

        let socket = socket.connect_binary(&socket_id,
            Callback::from({
                let link = link.clone();
                move |msg: Binary| {
                    match msg {
                        Ok(buff) => {
                            link.send_message(MonitorMsg::FragmentReceive(buff));
                        }
                        Err(e) => {
                            crate::log!("monitor recv error: {:?}", e);
                        }
                    }
                }
            }),
            Callback::from(|status: WebSocketStatus| {
                crate::log!("websocket status: {:?}", status);
            }))
        .expect("websocket.connect_binary");

        let media_source = MediaSource::new().unwrap();
        let source_url = Url::create_object_url_with_source(&media_source).unwrap();

        let source_open_event = EventListener::new(&media_source, "sourceopen", {
            let link = link.clone();
            move |_| link.send_message(MonitorMsg::SourceOpen)
        });

        Monitor {
            link,
            props,
            socket,
            media_source,
            source_buffer: None,
            fragment: Vec::new(),
            received: 0,
            ready: false,
            source_url,
            video_element: NodeRef::default(),
            _source_open_event: source_open_event,
            _buffer_ready_event: None,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            MonitorMsg::SourceOpen => {
                let source_buffer = self.media_source
                    // .add_source_buffer(r#"video/mp4; codecs="avc1.42E01E, mp4a.40.2""#)
                    .add_source_buffer(r#"video/mp4; codecs="avc1.42E01E, mp4a.40.2""#)
                    .unwrap();

                self._buffer_ready_event = Some(EventListener::new(
                    &source_buffer, "update", {
                        let link = self.link.clone();
                        move |_| {
                            // crate::log!("update fired!");
                            link.send_message(MonitorMsg::SourceBufferUpdate)
                        }
                    }));

                self.source_buffer = Some(source_buffer);
                self.ready = true;
                self.ready();
                false
            }
            MonitorMsg::SourceBufferUpdate => {
                self.ready = true;
                self.ready();
                false
            }
            MonitorMsg::FragmentReceive(fragment) => {
                self.received += fragment.len();
                self.fragment.extend(fragment);
                self.ready();
                false
            }
        }
    }

    fn mounted(&mut self) -> ShouldRender {
        true
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        html! {
            <video width={400} height={250} ref={self.video_element.clone()} src={&self.source_url} controls={true} />
        }
    }
}

impl Monitor {
    fn ready(&mut self) {
        if self.ready /*&& self.received >= 8*1024*/ {
            let mut fragment = mem::take(&mut self.fragment);
            self.append_buffer(&mut fragment);
            self.ready = false;
        }
    }

    fn append_buffer(&mut self, fragment: &mut [u8]) {
        if let Some(source_buffer) = &mut self.source_buffer {
            match source_buffer.append_buffer_with_u8_array(fragment) {
                Ok(()) => {
                    if let Some(video) = self.video_element.cast::<HtmlVideoElement>() {
                        let buffered = video.buffered();
                        let range_count = buffered.length();
                        let buffered_until =
                            if range_count > 0 {
                                buffered.end(range_count - 1).unwrap_or(0.0)
                            } else {
                                0.0
                            };

                        if buffered_until > 0.2 {
                            if video.paused() {
                                video.play();
                            }
                        }
                    }
                }
                Err(e) => {
                    panic!("append_buffer: {:?}", e);
                }
            }
        }
    }
}
