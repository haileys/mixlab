use std::mem;
use yew::{Component, ComponentLink, Html, ShouldRender, Properties};
use mixlab_protocol::ModuleId;
use crate::workspace::Window;

pub trait PureModule: Clone + PartialEq + 'static {
    fn view(&self, id: ModuleId, module: ComponentLink<Window>) -> Html;
}

#[derive(Properties, Clone)]
pub struct PureProps<Params: PureModule> {
    pub id: ModuleId,
    pub params: Params,
    pub module: ComponentLink<Window>,
}

pub struct Pure<Params: PureModule> {
    props: PureProps<Params>
}

impl<Params: PureModule> Component for Pure<Params> {
    type Properties = PureProps<Params>;
    type Message = ();

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Self { props }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, mut props: Self::Properties) -> ShouldRender {
        mem::swap(&mut self.props, &mut props);

        self.props.id != props.id || self.props.params != props.params
    }

    fn view(&self) -> Html {
        self.props.params.view(self.props.id, self.props.module.clone())
    }
}
