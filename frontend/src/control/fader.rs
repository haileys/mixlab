use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d, MouseEvent};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef, Callback};

use crate::component::drag_target::{DragTarget, DragEvent};
use crate::component::scroll_target::{Scroll, ScrollTarget};
use crate::util;

const FADER_WIDTH: usize = 64;
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

struct DragState {
    origin_y: i32,
    fader_value: f64,
}

enum MouseMode {
    Normal,
    Hover,
    Drag(DragState),
}

#[derive(Properties, Clone)]
pub struct FaderProps {
    pub value: f64,
    pub onchange: Callback<f64>,
}

pub enum FaderMsg {
    MouseMove(MouseEvent),
    DragStart(DragEvent),
    Drag(DragEvent),
    DragEnd(DragEvent),
    Scroll(Scroll)
}

impl Fader {
    fn fader_value(&self) -> f64 {
        match &self.mouse_mode {
            MouseMode::Normal | MouseMode::Hover =>
                self.props.value,
            MouseMode::Drag(drag_state) =>
                drag_state.fader_value,
        }
    }

    fn fader_handle_offset_top(&self) -> f64 {
        FADER_SHAFT_HEIGHT as f64 * (1.0 - self.fader_value())
    }

    fn drag_event(&mut self, ev: DragEvent) -> ShouldRender {
        let origin_y = match &self.mouse_mode {
            MouseMode::Normal | MouseMode::Hover => {
                let origin_y = ev.offset_y;

                let handle_y = self.fader_handle_offset_top();
                let midpoint_y = handle_y + FADER_HANDLE_HEIGHT as f64 / 2.0;

                let origin_y = origin_y - midpoint_y as i32;

                self.mouse_mode = MouseMode::Drag(DragState {
                    origin_y,
                    fader_value: self.props.value,
                });

                origin_y
            }
            MouseMode::Drag(drag_state) => drag_state.origin_y,
        };

        let new_fader_y = ev.offset_y - origin_y;

        let position = (new_fader_y - FADER_SHAFT_OFFSET_TOP as i32) as f64
            / FADER_SHAFT_HEIGHT as f64;

        let fader_value = util::clamp(0.0, 1.0, 1.0 - position);

        self.mouse_mode = MouseMode::Drag(DragState { origin_y, fader_value });
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
                        let y = ev.offset_y() as f64;
                        let fader_y = self.fader_handle_offset_top();

                        if y >= fader_y && y < fader_y + FADER_HANDLE_HEIGHT as f64 {
                            self.mouse_mode = MouseMode::Hover;
                        } else {
                            self.mouse_mode = MouseMode::Normal;
                        }

                        true
                    }
                    MouseMode::Drag { .. } => false,
                }
            }
            FaderMsg::DragStart(ev) => {
                self.drag_event(ev)
            }
            FaderMsg::Drag(ev) => {
                self.drag_event(ev)
            }
            FaderMsg::DragEnd(ev) => {
                self.drag_event(ev);
                self.mouse_mode = MouseMode::Normal;
                true
            }
            FaderMsg::Scroll(dir) => {
                let delta = match dir {
                    Scroll::Up(delta) => delta,
                    Scroll::Down(delta) => delta * -1.0,
                };
                let factor = 0.0001;
                let fader_value = util::clamp(0.0, 1.0, self.props.value + delta * factor);
                self.props.onchange.emit(fader_value);
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
            let line_y = (fader_y + FADER_HANDLE_HEIGHT as f64 / 2.0).floor() + 0.5;
            ctx.set_stroke_style(&JsValue::from_str("#f0f0f5"));
            ctx.begin_path();
            ctx.move_to(0.0, line_y as f64);
            ctx.line_to(FADER_WIDTH as f64, line_y as f64);
            ctx.stroke();
        }

        let canvas_style = match self.mouse_mode {
            MouseMode::Normal => "",
            MouseMode::Hover => "cursor:grab;",
            MouseMode::Drag { .. } => "cursor:grabbing;"
        };

        html! {
            <div class="control-fader">
                <ScrollTarget
                    on_scroll={self.link.callback(FaderMsg::Scroll)}
                >
                    <DragTarget
                        on_drag_start={self.link.callback(FaderMsg::DragStart)}
                        on_drag={self.link.callback(FaderMsg::Drag)}
                        on_drag_end={self.link.callback(FaderMsg::DragEnd)}
                    >
                        <canvas
                            width={FADER_WIDTH}
                            height={FADER_HEIGHT}
                            ref={self.canvas.clone()}
                            style={canvas_style}
                            onmousemove={self.link.callback(FaderMsg::MouseMove)}
                        />
                    </DragTarget>
                </ScrollTarget>
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
