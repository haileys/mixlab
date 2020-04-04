use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d, MouseEvent};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef, Callback};

const FADER_WIDTH: usize = 40;
const FADER_HEIGHT: usize = 160;
const FADER_HANDLE_HEIGHT: usize = 17; // always odd to account for line in the middle
const FADER_SHAFT_HEIGHT: usize = FADER_HEIGHT - FADER_HANDLE_HEIGHT;
const FADER_SHAFT_WIDTH: usize = 4;
const FADER_SHAFT_OFFSET_TOP: usize = FADER_HANDLE_HEIGHT / 2;
const FADER_NOTCH_INTERVAL: usize = 11;

pub struct Fader {
    link: ComponentLink<Self>,
    props: FaderProps,
    canvas: NodeRef,
    ctx: Option<CanvasRenderingContext2d>,
    mouse_mode: MouseMode,
}

#[derive(PartialEq, Clone, Copy)]
enum MouseMode {
    Normal,
    Hover,
    Drag {
        offset_y: i32,
        fader_value: f32,
    }
}

#[derive(Properties, Clone)]
pub struct FaderProps {
    pub value: f32,
    pub onchange: Callback<f32>,
}

pub enum FaderMsg {
    MouseMove(MouseEvent),
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
}

impl Fader {
    fn fader_value(&self) -> f32 {
        match self.mouse_mode {
            MouseMode::Normal | MouseMode::Hover =>
                self.props.value,
            MouseMode::Drag { fader_value, .. } =>
                fader_value,
        }
    }

    fn fader_handle_offset_top(&self) -> f32 {
        FADER_SHAFT_HEIGHT as f32 * (1.0 - self.fader_value())
    }

    fn drag_event(&mut self, ev: MouseEvent) -> ShouldRender {
        let offset_y = match self.mouse_mode {
            MouseMode::Normal | MouseMode::Hover => {
                let handle_y = self.fader_handle_offset_top();
                crate::log!("handle_y = {}", handle_y);
                let midpoint_y = handle_y + FADER_HANDLE_HEIGHT as f32 / 2.0;
                crate::log!("midpoint_y = {}", midpoint_y);
                crate::log!("ev_offset_y = {}", ev.offset_y());
                ev.offset_y() - midpoint_y as i32
            }
            MouseMode::Drag { offset_y, .. } => offset_y,
        };

        let new_fader_y = ev.offset_y() - offset_y;

        let position = (new_fader_y - FADER_SHAFT_OFFSET_TOP as i32) as f32
            / FADER_SHAFT_HEIGHT as f32;

        let fader_value = 1.0 - position;

        let fader_value = if fader_value < 0.0 {
            0.0
        } else if fader_value > 1.0 {
            1.0
        } else {
            fader_value
        };

        self.mouse_mode = MouseMode::Drag { offset_y, fader_value };
        crate::log!("fader_value = {}", fader_value);
        self.props.onchange.emit(fader_value);

        true
    }
}

impl Component for Fader {
    type Properties = FaderProps;
    type Message = FaderMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Fader {
            link,
            props,
            canvas: NodeRef::default(),
            ctx: None,
            mouse_mode: MouseMode::Normal,
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            FaderMsg::MouseMove(ev) => {
                match self.mouse_mode {
                    MouseMode::Normal | MouseMode::Hover => {
                        let y = ev.offset_y() as f32;
                        let fader_y = self.fader_handle_offset_top();

                        let prev_mouse_mode = self.mouse_mode;

                        if y >= fader_y && y < fader_y + FADER_HANDLE_HEIGHT as f32 {
                            self.mouse_mode = MouseMode::Hover;
                        } else {
                            self.mouse_mode = MouseMode::Normal;
                        }

                        prev_mouse_mode != self.mouse_mode
                    }
                    MouseMode::Drag { .. } => {
                        self.drag_event(ev)
                    }
                }
            }
            FaderMsg::MouseDown(ev) => {
                self.drag_event(ev)
            }
            FaderMsg::MouseUp(ev) => {
                self.drag_event(ev);
                self.mouse_mode = MouseMode::Normal;
                true
            }
        }
    }

    fn view(&self) -> Html {
        if let Some(ctx) = &self.ctx {
            ctx.clear_rect(0f64, 0f64, FADER_WIDTH as f64, FADER_HEIGHT as f64);

            // set fill and stroke for shaft and notches
            let style = JsValue::from_str("#f0f0f5");
            ctx.set_fill_style(&style);
            ctx.set_stroke_style(&style);

            // draw central shaft
            ctx.begin_path();
            ctx.rect(
                ((FADER_WIDTH - FADER_SHAFT_WIDTH) / 2) as f64,
                FADER_SHAFT_OFFSET_TOP as f64,
                FADER_SHAFT_WIDTH as f64,
                FADER_SHAFT_HEIGHT as f64,
            );
            ctx.fill();

            // draw notches
            for y in (0..=FADER_SHAFT_HEIGHT).step_by(FADER_NOTCH_INTERVAL) {
                let y = (FADER_SHAFT_OFFSET_TOP + y) as f64 + 0.5;
                ctx.begin_path();
                ctx.move_to(0.0, y);
                ctx.line_to(FADER_WIDTH as f64, y);
                ctx.stroke();
            }

            // draw fader handle
            let fader_y = self.fader_handle_offset_top();
            ctx.set_fill_style(&JsValue::from_str("#8d8bb0"));
            ctx.begin_path();
            ctx.rect(
                0.0,
                fader_y as f64,
                FADER_WIDTH as f64,
                FADER_HANDLE_HEIGHT as f64,
            );
            ctx.fill();

            // draw center line on fader handle
            let line_y = (fader_y + FADER_HANDLE_HEIGHT as f32 / 2.0).floor() + 0.5;
            ctx.set_stroke_style(&JsValue::from_str("#f0f0f5"));
            ctx.begin_path();
            ctx.move_to(0.0, line_y as f64);
            ctx.line_to(FADER_WIDTH as f64, line_y as f64);
            ctx.stroke();
        }

        let canvas_style = match self.mouse_mode {
            MouseMode::Normal => "",
            MouseMode::Hover | MouseMode::Drag { .. } => "cursor:pointer;"
        };

        html! {
            <div class="control-fader">
                <canvas
                    width={FADER_WIDTH}
                    height={FADER_HEIGHT}
                    ref={self.canvas.clone()}
                    onmousemove={self.link.callback(FaderMsg::MouseMove)}
                    onmousedown={self.link.callback(FaderMsg::MouseDown)}
                    onmouseup={self.link.callback(FaderMsg::MouseUp)}
                    style={canvas_style}
                />
            </div>
        }
    }

    fn mounted(&mut self) -> ShouldRender {
        if let Some(canvas) = self.canvas.cast::<HtmlCanvasElement>() {
            let ctx = canvas.get_context("2d")
                .expect("canvas.get_context")
                .expect("canvas.get_context");

            let ctx = ctx
                .dyn_into::<CanvasRenderingContext2d>()
                .expect("dyn_ref::<CanvasRenderingContext2d>");

            self.ctx = Some(ctx);
        }

        true
    }
}