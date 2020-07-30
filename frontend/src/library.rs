use uuid::Uuid;
use web_sys::File;
use yew::events::ChangeData;
use yew::{html, Callback, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

pub struct MediaLibrary {
    link: ComponentLink<Self>,
    items: Vec<MediaItem>
}

pub struct MediaItem {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub size: usize,
}

pub enum LibraryMsg {
    UploadFiles(Vec<File>),
}

impl Component for MediaLibrary {
    type Message = LibraryMsg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        MediaLibrary {
            link,
            items: vec![
                MediaItem {
                    id: Uuid::new_v4(),
                    name: "Real Scenes - Melbourne _ Resident Advisor-cs1Iw-r0YI8.mp4".to_string(),
                    kind: "video/mp4".to_string(),
                    size: 635_952_409,
                },
                MediaItem {
                    id: Uuid::new_v4(),
                    name: "Tron.Legacy.BluRay.1080p.x264.5.1.Judas.mp4".to_string(),
                    kind: "video/mp4".to_string(),
                    size: 2_955_571_205,
                },
            ]
        }
    }

    fn change(&mut self, _: Self::Properties) -> ShouldRender {
        false
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            LibraryMsg::UploadFiles(_) => {
            }
        }
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="media-library">
                <div class="media-library-main-button-row">
                    <UploadButton onfileupload={self.link.callback(LibraryMsg::UploadFiles)} />
                </div>
                <table class="media-library-list">
                    <tr class="table-heading">
                        <th>{"Name"}</th>
                        <th>{"Kind"}</th>
                        <th>{"Size"}</th>
                    </tr>
                    { for self.items.iter().map(|item| {
                        html! {
                            <tr>
                                <td>{&item.name}</td>
                                <td>{&item.kind}</td>
                                <td>{format_size(item.size)}</td>
                            </tr>
                        }
                    }) }
                </table>
            </div>
        }
    }
}

fn format_size(bytes: usize) -> String {
    const KIB: usize = 1024;
    const MIB: usize = 1024 * 1024;
    const GIB: usize = 1024 * 1024 * 1024;

    if bytes == 1 {
        "1 byte".to_string()
    } else if bytes < KIB {
        format!("{} bytes", bytes)
    } else if bytes < MIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else if bytes < GIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    }
}

#[derive(Properties, Clone)]
struct UploadButtonProps {
    onfileupload: Callback<Vec<File>>,
}

struct UploadButton {
    props: UploadButtonProps,
    link: ComponentLink<Self>,
    input_ref: NodeRef,
}

enum UploadButtonMsg {
    FileSelected(ChangeData),
}

impl Component for UploadButton {
    type Properties = UploadButtonProps;
    type Message = UploadButtonMsg;

    fn create(props: UploadButtonProps, link: ComponentLink<Self>) -> Self {
        UploadButton {
            props,
            link,
            input_ref: NodeRef::default(),
        }
    }

    fn change(&mut self, props: UploadButtonProps) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: UploadButtonMsg) -> ShouldRender {
        match msg {
            UploadButtonMsg::FileSelected(ev) => {
                let file_list = match ev {
                    ChangeData::Files(file_list) => file_list,
                    _ => {
                        // should never happen
                        return false;
                    }
                };

                let mut files = Vec::new();

                for i in 0..file_list.length() {
                    if let Some(file) = file_list.get(i) {
                        files.push(file);
                    }
                }

                self.props.onfileupload.emit(files);
                false
            }
        }
    }

    fn view(&self) -> Html {
        html! {
            <>
                <label>
                    <div class="media-library-main-button">{"+ Upload"}</div>
                    <input
                        type="file"
                        ref={self.input_ref.clone()}
                        style="display:none"
                        onchange={self.link.callback(UploadButtonMsg::FileSelected)}
                    />
                </label>
            </>
        }
    }
}
