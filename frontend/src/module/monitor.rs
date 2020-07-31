use std::mem;

use gloo_events::EventListener;
use web_sys::{MediaSource, SourceBuffer, Url, HtmlVideoElement};
use yew::format::Binary;
use yew::services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef, Callback};

use mixlab_mux::mp4::Mp4Mux;
use mixlab_protocol::{ModuleId, MonitorIndication, MonitorTransportPacket};

use crate::util;

#[derive(Properties, Clone, Debug)]
pub struct MonitorProps {
    pub id: ModuleId,
    pub indication: MonitorIndication,
}

pub struct Monitor {
    link: ComponentLink<Self>,
    props: MonitorProps,
    state: MonitorState,
    socket_url: String,
    video_element: NodeRef,
    _source_open_event: Option<EventListener>,
}

pub enum MonitorMsg {
    OverlayClick,
    SourceOpen(SourceOpen),
    SourceBufferUpdate,
    PacketReceive(Vec<u8>),
}

pub enum MonitorState {
    Stopped,
    Loading(EventListener),
    Playing(PlayState),
}

pub struct PlayState {
    source_buffer: SourceBuffer,
    ready: bool,
    mux: Option<Mp4Mux>,
    fragment: Vec<u8>,
    _socket: WebSocketTask,
    _buffer_ready_event: EventListener,
}

pub struct SourceOpen {
    media_source: MediaSource,
    socket: WebSocketTask,
}

impl Component for Monitor {
    type Properties = MonitorProps;
    type Message = MonitorMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let socket_url = format!("{}/_monitor/{}", util::websocket_origin(), props.indication.socket_id);

        Monitor {
            link,
            props,
            socket_url,
            state: MonitorState::Stopped,
            video_element: NodeRef::default(),
            _source_open_event: None,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            MonitorMsg::OverlayClick => {
                match self.state {
                    MonitorState::Stopped => {
                        let media_source = MediaSource::new().unwrap();
                        let source_url = Url::create_object_url_with_source(&media_source).unwrap();

                        if let Some(video) = self.video_element.cast::<HtmlVideoElement>() {
                            video.set_src(&source_url);

                            // TODO - how should we deal with errors here?
                            let _ = video.play();
                        }

                        let source_open_event = EventListener::new(&media_source, "sourceopen", {
                            let link = self.link.clone();
                            let socket_url = self.socket_url.clone();
                            let media_source = media_source.clone();

                            move |_| {
                                let socket = WebSocketService::connect_binary(&socket_url,
                                    Callback::from({
                                        let link = link.clone();
                                        move |msg: Binary| {
                                            match msg {
                                                Ok(buff) => {
                                                    link.send_message(MonitorMsg::PacketReceive(buff));
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

                                link.send_message(MonitorMsg::SourceOpen(SourceOpen {
                                    media_source: media_source.clone(),
                                    socket,
                                }))
                            }
                        });

                        self.state = MonitorState::Loading(source_open_event);
                    }
                    MonitorState::Loading(_) => {
                        self.state = MonitorState::Stopped;
                    }
                    MonitorState::Playing(_) => {
                        self.state = MonitorState::Stopped;
                    }
                }

                true
            }
            MonitorMsg::SourceOpen(source_open) => {
                let source_buffer = source_open.media_source
                    // .add_source_buffer(r#"video/mp4; codecs="avc1.42E01E, mp4a.40.2""#)
                    .add_source_buffer(r#"video/mp4; codecs="avc1.42E01E, mp4a.40.2""#)
                    .unwrap();

                let buffer_ready_event = EventListener::new(
                    &source_buffer, "update", {
                        let link = self.link.clone();
                        move |_| {
                            link.send_message(MonitorMsg::SourceBufferUpdate)
                        }
                    });

                self.state = MonitorState::Playing(PlayState {
                    source_buffer,
                    mux: None,
                    ready: true,
                    fragment: Vec::new(),
                    _socket: source_open.socket,
                    _buffer_ready_event: buffer_ready_event,
                });

                false
            }
            MonitorMsg::SourceBufferUpdate => {
                if let MonitorState::Playing(play_state) = &mut self.state {
                    play_state.ready = true;
                    play_state.ready();
                }
                false
            }
            MonitorMsg::PacketReceive(packet) => {
                if let MonitorState::Playing(play_state) = &mut self.state {
                    let packet = bincode::deserialize::<MonitorTransportPacket>(&packet).unwrap();

                    match packet {
                        MonitorTransportPacket::Init { params } => {
                            if play_state.mux.is_some() {
                                panic!("protocol violation: received >1 init packet");
                            }

                            let (mux, init) = Mp4Mux::new(params);
                            play_state.mux = Some(mux);
                            play_state.fragment.extend(init);
                            play_state.ready()
                        }
                        MonitorTransportPacket::Frame { duration, track_data } => {
                            let mux = play_state.mux.as_mut()
                                .expect("protocol violation: received frame before init packet");

                            let segment = mux.write_track(duration, &track_data);
                            play_state.fragment.extend(segment);
                            play_state.ready()
                        }
                    }
                }

                false
            }
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        let overlay_class = match self.state {
            MonitorState::Stopped => "monitor-overlay monitor-overlay-stopped",
            MonitorState::Loading(_) => "monitor-overlay",
            MonitorState::Playing(_) => "monitor-overlay monitor-overlay-playing",
        };

        let overlay_icon = match self.state {
            MonitorState::Stopped => html! {
                <svg width={64} height={64}>
                    <polygon points="8,0 56,32 8,64" fill="#ffffff" />
                </svg>
            },
            MonitorState::Loading(_) => html! {},
            MonitorState::Playing(_) => html! {
                <svg width={64} height={64}>
                    <rect width={64} height={64} fill="#ffffff" />
                </svg>
            }
        };

        html! {
            <div class="monitor-container">
                <div class={overlay_class} onclick={self.link.callback(|_| MonitorMsg::OverlayClick)}>
                    <div class="monitor-overlay-icon">
                        {overlay_icon}
                    </div>
                </div>
                <video width={400} height={250} ref={self.video_element.clone()} class="monitor-video" />
            </div>
        }
    }
}

impl PlayState {
    fn ready(&mut self) {
        if self.ready {
            let mut fragment = mem::take(&mut self.fragment);

            if fragment.len() > 0 {
                self.append_buffer(&mut fragment);
                self.ready = false;
            }
        }
    }

    fn append_buffer(&mut self, fragment: &mut [u8]) {
        self.source_buffer.append_buffer_with_u8_array(fragment).expect("append_buffer");
    }
}
