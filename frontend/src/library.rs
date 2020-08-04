use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::num::NonZeroUsize;

use gloo_events::EventListener;
use http::request::Request;
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{File, XmlHttpRequest, ProgressEvent};
use yew::events::ChangeData;
use yew::{html, Callback, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

use crate::util::{self, Sequence};

pub struct MediaLibrary {
    link: ComponentLink<Self>,
    upload_seq: Sequence,
    uploads: BTreeMap<NonZeroUsize, InProgressUpload>,
    items: Vec<MediaItem>
}

pub struct MediaItem {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub size: usize,
}

pub enum LibraryMsg {
    SelectFiles(Vec<File>),
    Upload(NonZeroUsize, UploadEvent),
}

impl Component for MediaLibrary {
    type Message = LibraryMsg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        MediaLibrary {
            link,
            upload_seq: Sequence::new(),
            uploads: BTreeMap::new(),
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
            LibraryMsg::SelectFiles(files) => {
                for file in files {
                    let id = self.upload_seq.next();

                    let filename = file.name();

                    let task = UploadTask::start(file,
                        self.link.callback(move |ev|
                            LibraryMsg::Upload(id, ev))
                    ).expect("UploadTask::start");

                    self.uploads.insert(id, InProgressUpload {
                        filename,
                        progress: None,
                        task,
                    });
                }

                true
            }
            LibraryMsg::Upload(id, event) => {
                match event {
                    UploadEvent::Progress(progress) => {
                        if let Some(upload) = self.uploads.get_mut(&id) {
                            upload.progress = Some(progress);
                            true
                        } else {
                            false
                        }
                    }
                    UploadEvent::Complete => {
                        self.uploads.remove(&id);
                        true
                    }
                }
            }
        }
    }

    fn view(&self) -> Html {
        html! {
            <div class="media-library">
                <div class="media-library-main-button-row">
                    <UploadButton on_file_upload={self.link.callback(LibraryMsg::SelectFiles)} />
                </div>
                { if self.uploads.is_empty() {
                    html! {}
                } else {
                    html! {
                        <table class="media-library-table">
                            <tr class="table-heading">
                                <th>{"Uploads"}</th>
                            </tr>
                            { for self.uploads.iter().map(|(id, item)| {
                                html! {
                                    <>
                                        <tr>
                                            <td>{&item.filename}</td>
                                            <td class="media-library-upload-progress-percent">
                                                { match &item.progress {
                                                    Some(progress) => format!("{:.1}%", progress.as_percent()),
                                                    None => "".to_string(),
                                                } }
                                            </td>
                                        </tr>
                                        <tr class="media-library-upload-progress-row">
                                            <td colspan={2}>
                                                { match &item.progress {
                                                    Some(progress) => html! {
                                                        <progress max={progress.total} value={progress.uploaded} />
                                                    },
                                                    None => html!{
                                                        <progress />
                                                    },
                                                } }
                                            </td>
                                        </tr>
                                    </>
                                }
                            }) }
                        </table>
                    }
                } }
                <table class="media-library-table">
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
    on_file_upload: Callback<Vec<File>>,
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

                self.props.on_file_upload.emit(files);
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

struct InProgressUpload {
    filename: String,
    progress: Option<UploadProgress>,
    task: UploadTask,
}

pub struct UploadProgress {
    uploaded: u64,
    total: u64,
}

impl UploadProgress {
    fn as_percent(&self) -> f64 {
        (self.uploaded as f64 / self.total as f64) * 100.0
    }
}

struct UploadTask {
    xhr: XmlHttpRequest,
    _progress_event: EventListener,
    _load_event: EventListener,
}

pub enum UploadEvent {
    Progress(UploadProgress),
    Complete,
}

impl UploadTask {
    fn start(file: File, callback: Callback<UploadEvent>) -> Result<UploadTask, JsValue> {
        crate::log!("origin: {:?}", util::origin());
        let url = util::origin() + "/_upload/" + &file.name();

        let mut kind = file.type_();
        if kind == "" {
            kind = "application/octet-stream".to_string();
        }

        let xhr = XmlHttpRequest::new()?;
        let upload = xhr.upload()?;

        let progress_event = EventListener::new(&upload, "progress", {
            let callback = callback.clone();
            move |ev| {
                if let Some(ev) = ev.dyn_ref::<ProgressEvent>() {
                    let uploaded = ev.loaded() as u64;
                    let total = ev.total() as u64;
                    callback.emit(UploadEvent::Progress(UploadProgress {
                        uploaded,
                        total,
                    }));
                }
            }
        });

        let load_event = EventListener::new(&upload, "load", {
            let callback = callback.clone();
            move |_| { callback.emit(UploadEvent::Complete); }
        });

        xhr.open("POST", &url)?;
        xhr.override_mime_type(&kind)?;
        xhr.send_with_opt_blob(Some(&file))?;

        Ok(UploadTask {
            xhr,
            _progress_event: progress_event,
            _load_event: load_event,
        })
    }
}

impl Drop for UploadTask {
    fn drop(&mut self) {
        /// https://developer.mozilla.org/en-US/docs/Web/API/XMLHttpRequest/readyState
        const DONE: u16 = 4;

        if self.xhr.ready_state() != DONE {
            // nothing we can do in drop if abort fails
            let _ = self.xhr.abort();
        }
    }
}
