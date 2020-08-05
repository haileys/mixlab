#![recursion_limit="1024"]

mod component;
mod control;
mod library;
mod module;
mod service;
mod session;
mod sidebar;
mod util;
mod workspace;

use std::fmt::Display;

use derive_more::Display;
use wasm_bindgen::prelude::*;
use yew::{html, Component, ComponentLink, Html, ShouldRender, Callback, Properties};

use mixlab_protocol::WorkspaceOp;

use library::MediaLibrary;
use session::{Session, SessionRef};
use sidebar::Sidebar;
use util::{notify, Sequence};
use workspace::Workspace;

pub struct App {
    link: ComponentLink<Self>,
    session: SessionRef,
    selected_tab: Tab,
}

#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Tab {
    #[display(fmt = "Workspace")]
    Workspace,
    #[display(fmt = "Media Library")]
    MediaLibrary,
}

#[derive(Debug)]
pub enum AppMsg {
    ClientUpdate(WorkspaceOp),
    ChangeTab(Tab),
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        App {
            link,
            session: Session::new(),
            selected_tab: Tab::Workspace,
        }
    }

    fn change(&mut self, _: ()) -> ShouldRender {
        false
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            AppMsg::ClientUpdate(op) => {
                self.session.update_workspace(op);
                false
            }
            AppMsg::ChangeTab(tab) => {
                self.selected_tab = tab;
                true
            }
        }
    }

    fn view(&self) -> Html {
        html! {
            <div class="app">
                <SidebarContainer session={self.session.clone()} />

                <div class="main">
                    <TabBar<Tab>
                        current={self.selected_tab.clone()}
                        tabs={vec![
                            Tab::Workspace,
                            Tab::MediaLibrary,
                        ]}
                        onchange={self.link.callback(AppMsg::ChangeTab)}
                    />

                    { match self.selected_tab {
                        Tab::Workspace => html! {
                            <WorkspaceContainer
                                app={self.link.clone()}
                                session={self.session.clone()}
                            />
                        },
                        Tab::MediaLibrary => html! {
                            <MediaLibrary />
                        },
                    } }
                </div>
            </div>
        }
    }
}

pub struct WorkspaceContainer {
    _notify: notify::Handle,
    props: WorkspaceContainerProps,
}

#[derive(Properties, Clone)]
pub struct WorkspaceContainerProps {
    app: ComponentLink<App>,
    session: SessionRef,
}

impl Component for WorkspaceContainer {
    type Message = ();
    type Properties = WorkspaceContainerProps;

    fn create(props: WorkspaceContainerProps, link: ComponentLink<Self>) -> Self {
        let notify = props.session.listen_workspace(link.callback(|()| ()));

        WorkspaceContainer {
            _notify: notify,
            props,
        }
    }

    fn change(&mut self, new_props: WorkspaceContainerProps) -> ShouldRender {
        // TODO do we need to resubscribe?
        self.props = new_props;
        true
    }

    fn update(&mut self, _: ()) -> ShouldRender {
        true
    }

    fn view(&self) -> Html {
        if let Some(state) = self.props.session.workspace() {
            html! {
                <Workspace
                    app={self.props.app.clone()}
                    state={state.clone()}
                />
            }
        } else {
            html! {}
        }
    }
}

pub struct SidebarContainer {
    _notify: notify::Handle,
    props: SidebarContainerProps,
}

#[derive(Properties, Clone)]
pub struct SidebarContainerProps {
    session: SessionRef,
}

impl Component for SidebarContainer {
    type Message = ();
    type Properties = SidebarContainerProps;

    fn create(props: SidebarContainerProps, link: ComponentLink<Self>) -> Self {
        let notify = props.session.listen_workspace(link.callback(|()| ()));

        SidebarContainer {
            _notify: notify,
            props,
        }
    }

    fn change(&mut self, new_props: SidebarContainerProps) -> ShouldRender {
        // TODO do we need to resubscribe?
        self.props = new_props;
        true
    }

    fn update(&mut self, _: ()) -> ShouldRender {
        true
    }

    fn view(&self) -> Html {
        if let Some(state) = self.props.session.workspace() {
            html! {
                <Sidebar
                    session={self.props.session.clone()}
                    workspace={state.clone()}
                />
            }
        } else {
            html! {}
        }
    }
}

#[derive(Properties, Clone, Debug)]
pub struct TabBarProps<T: Clone> {
    current: T,
    tabs: Vec<T>,
    onchange: Callback<T>,
}

struct TabBar<T: Clone> {
    props: TabBarProps<T>
}

impl<T: Display + Clone + PartialEq + 'static> Component for TabBar<T> {
    type Properties = TabBarProps<T>;
    type Message = ();

    fn create(props: TabBarProps<T>, _: ComponentLink<Self>) -> Self {
        TabBar { props }
    }

    fn change(&mut self, props: TabBarProps<T>) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, _: ()) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="tab-bar">
                { for self.props.tabs.iter().map(|tab| {
                    let class = if tab == &self.props.current {
                        "tab-bar-tab tab-bar-active"
                    } else {
                        "tab-bar-tab"
                    };

                    html! {
                        <div
                            class={class}
                            onclick={self.props.onchange.reform({
                                let tab = tab.clone();
                                move |_| tab.clone()
                            })}
                        >
                            {tab.to_string()}
                        </div>
                    }
                }) }
            </div>
        }
    }
}

#[wasm_bindgen]
pub fn start() {
    console_error_panic_hook::set_once();

    yew::start_app::<App>();
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_str(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_val(v: &wasm_bindgen::JsValue);

    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn warn_str(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn error_str(s: &str);
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => (crate::log_str(&format_args!($($t)*).to_string()))
}

#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => (crate::warn_str(&format_args!($($t)*).to_string()))
}

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => (crate::error_str(&format_args!($($t)*).to_string()))
}
