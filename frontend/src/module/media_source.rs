use std::fmt::{self, Display};
use std::rc::Rc;

use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};
use yew_components::Select;

use mixlab_protocol::{ModuleId, ModuleParams, MediaSourceParams, MediaLibrary, MediaItem};

use crate::util::notify;
use crate::session::SessionRef;
use crate::workspace::{Window, WindowMsg};

#[derive(Properties, Clone, Debug)]
pub struct MediaSourceProps {
    pub id: ModuleId,
    pub module: ComponentLink<Window>,
    pub params: MediaSourceParams,
    pub session: SessionRef,
}

pub struct MediaSource {
    props: MediaSourceProps,
    link: ComponentLink<Self>,
    library: Option<Rc<MediaLibrary>>,
    _notify: notify::Handle,
}

pub enum MediaSourceMsg {
    MediaLibrary(Rc<MediaLibrary>),
    ChangeSource(MediaSourceItem),
}

impl Component for MediaSource {
    type Properties = MediaSourceProps;
    type Message = MediaSourceMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let notify = props.session.listen_media(link.callback(MediaSourceMsg::MediaLibrary));

        Self {
            props,
            link,
            library: None,
            _notify: notify,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            MediaSourceMsg::MediaLibrary(library) => {
                self.library = Some(library);
                true
            }
            MediaSourceMsg::ChangeSource(source) => {
                self.props.module.send_message(
                    WindowMsg::UpdateParams(
                        ModuleParams::MediaSource(
                            MediaSourceParams {
                                media_id: Some(source.0.id),
                                ..self.props.params.clone()
                            })));
                false
            }
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        let options = self.library.iter()
            .flat_map(|library| library.items.iter().cloned())
            .map(MediaSourceItem)
            .collect::<Vec<_>>();

        html! {
            <Select<MediaSourceItem>
                options={options}
                on_change={self.link.callback(MediaSourceMsg::ChangeSource)}
            />
        }
    }
}

#[derive(Clone)]
pub struct MediaSourceItem(MediaItem);

impl PartialEq for MediaSourceItem {
    fn eq(&self, other: &MediaSourceItem) -> bool {
        self.0.id == other.0.id
    }
}

impl Display for MediaSourceItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.name)
    }
}
