use yew::{html, ComponentLink, Html, ChangeData};

use mixlab_protocol::{ModuleId, ModuleParams, ShaderParams};

use crate::component::midi_target::MidiUiMode;
use crate::component::pure_module::{Pure, PureModule};
use crate::workspace::{Window, WindowMsg};

pub type Shader = Pure<ShaderParams>;

impl PureModule for ShaderParams {
    fn view(&self, _: ModuleId, module: ComponentLink<Window>, _: MidiUiMode) -> Html {

        html! {
            <>
                <textarea
                    onchange={module.callback(|change: ChangeData| {
                        match change {
                            ChangeData::Value(src) => {
                                WindowMsg::UpdateParams(
                                    ModuleParams::Shader(ShaderParams {
                                        fragment_shader_source: src,
                                    }))
                            }
                            _ => unreachable!()
                        }
                    })}
                >
                    {&self.fragment_shader_source}
                </textarea>
            </>
        }
    }
}
