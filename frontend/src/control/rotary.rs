use std::f64;
use std::fmt::Display;
use std::mem;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef, Callback};

use crate::component::drag_target::{DragTarget, DragEvent};
use crate::component::scroll_target::{Scroll, ScrollTarget};
use crate::util;

const ROTARY_WIDTH: usize = 48;
const ROTARY_HEIGHT: usize = 48;
const ROTARY_ADJUST_HEIGHT: usize = 200;

pub struct Rotary<T: Into<f64> + From<f64> + Clone + Copy + Display + PartialEq + 'static> {
    link: ComponentLink<Self>,
    props: RotaryProps<T>,
    canvas: NodeRef,
    mouse_mode: MouseMode,
}

#[derive(Properties, Clone, Debug)]
pub struct RotaryProps<T: Clone> {
    pub min: T,
    pub max: T,
    pub value: T,
    pub default: Option<T>,
    pub onchange: Callback<T>,
}

impl<T: Into<f64> + From<f64> + Clone + Copy> RotaryProps<T> {
    fn value_frac(&self) -> f64 {
        self.frac_for(self.value.into())
    }

    fn frac_for(&self, value: f64) -> f64 {
        (value - self.min.into()) / (self.max.into() - self.min.into())
    }
}

pub enum RotaryMsg {
    DragStart(DragEvent),
    Drag(DragEvent),
    DragEnd(DragEvent),
    Scroll(Scroll),
}

enum MouseMode {
    Normal,
    Drag(DragState),
}

#[derive(Debug)]
struct DragState {
    offset_x: i32,
    offset_y: i32,
    value: f64,
}

impl DragState {
    fn update_value<T: Into<f64> + From<f64> + Clone + Copy>(&mut self, props: &RotaryProps<T>, ev: &DragEvent) {
        let min_y = self.offset_y as f64 + props.value_frac() * ROTARY_ADJUST_HEIGHT as f64;
        let new_frac = (min_y - ev.offset_y as f64) / ROTARY_ADJUST_HEIGHT as f64;
        let new_frac = util::clamp(0.0, 1.0, new_frac);
        self.value = props.min.into() + new_frac * (props.max.into() - props.min.into());
    }
}

impl<T: Into<f64> + From<f64> + Clone + Copy + Display + PartialEq + 'static> Component for Rotary<T> {
    type Properties = RotaryProps<T>;
    type Message = RotaryMsg;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Rotary {
            link,
            props,
            canvas: NodeRef::default(),
            mouse_mode: MouseMode::Normal,
        }
    }

    fn change(&mut self, mut props: Self::Properties) -> ShouldRender {
        mem::swap(&mut self.props, &mut props);

        // only re-render if any UI influencing prop has changed
        let old = (props.min, props.max, props.value);
        let new = (self.props.min, self.props.max, self.props.value);
        old != new
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            RotaryMsg::DragStart(ev) => {
                self.mouse_mode = MouseMode::Drag(DragState {
                    offset_x: ev.offset_x,
                    offset_y: ev.offset_y,
                    value: self.props.value.into(),
                });
                false
            }
            RotaryMsg::Drag(ev) => {
                if let MouseMode::Drag(drag_state) = &mut self.mouse_mode {
                    drag_state.update_value(&self.props, &ev);
                    true
                } else {
                    false
                }
            }
            RotaryMsg::DragEnd(ev) => {
                if let MouseMode::Drag(drag_state) = &mut self.mouse_mode {
                    drag_state.update_value(&self.props, &ev);
                    self.props.onchange.emit(T::from(drag_state.value));
                    self.mouse_mode = MouseMode::Normal;
                    true
                } else {
                    false
                }
            }
            RotaryMsg::Scroll(dir) => {
                let delta = match dir {
                    Scroll::Up(delta) => delta,
                    Scroll::Down(delta) => delta * -1.0,
                };
                let factor = 0.0001;
                let new_frac = util::clamp(0.0, 1.0, factor * delta + self.props.value_frac());
                let value = T::from(
                    self.props.min.into() + new_frac * (self.props.max.into() - self.props.min.into())
                );
                self.props.onchange.emit(value);
                true
            }
        }
    }

    fn view(&self) -> Html {
        let label_value = match &self.mouse_mode {
            MouseMode::Normal => self.props.value,
            MouseMode::Drag(state) => T::from(state.value),
        };

        html! {
            <div class="control-rotary">
                <ScrollTarget
                    on_scroll={self.link.callback(RotaryMsg::Scroll)}
                >
                    <DragTarget
                        on_drag_start={self.link.callback(RotaryMsg::DragStart)}
                        on_drag={self.link.callback(RotaryMsg::Drag)}
                        on_drag_end={self.link.callback(RotaryMsg::DragEnd)}
                    >
                        <canvas
                            width={ROTARY_WIDTH}
                            height={ROTARY_HEIGHT}
                            ref={self.canvas.clone()}
                        />
                    </DragTarget>
                    <div class="control-rotary-label">{format!("{}", label_value)}</div>
                </ScrollTarget>
            </div>

        }
    }

    fn rendered(&mut self, _: bool) {
        if let Some(canvas) = self.canvas.cast::<HtmlCanvasElement>() {
            let ctx = canvas.get_context("2d")
                .expect("canvas.get_context")
                .expect("canvas.get_context");

            let ctx = ctx
                .dyn_into::<CanvasRenderingContext2d>()
                .expect("dyn_ref::<CanvasRenderingContext2d>");

            const ROTARY_CENTER_X: f64 = ROTARY_WIDTH as f64 / 2.0;
            const ROTARY_CENTER_Y: f64 = ROTARY_HEIGHT as f64 / 2.0;
            const ROTARY_RADIUS: f64 = ROTARY_WIDTH as f64 / 2.0;
            const ROTARY_RING_WIDTH: f64 = 2.0;
            const ROTARY_HAND_WIDTH: f64 = 4.0;

            ctx.clear_rect(0f64, 0f64, ROTARY_WIDTH as f64, ROTARY_HEIGHT as f64);

            let value_frac = match &self.mouse_mode {
                MouseMode::Normal => self.props.value_frac(),
                MouseMode::Drag(state) => self.props.frac_for(state.value),
            };
            let start_angle = f64::consts::PI * 2.0 / 3.0;
            let end_angle = f64::consts::PI * 1.0 / 3.0;
            let angular_distance = 2.0 * f64::consts::PI * 5.0 / 6.0;
            let value_angle = start_angle + (value_frac * angular_distance);

            // draw outer ring

            ctx.begin_path();
            ctx.set_stroke_style(&JsValue::from_str("#f0f0f5"));
            ctx.set_line_width(2.0);
            let _ = ctx.arc(
                ROTARY_CENTER_X,
                ROTARY_CENTER_Y,
                ROTARY_RADIUS - ROTARY_RING_WIDTH / 2.0,
                start_angle,
                end_angle,
            );
            ctx.stroke();

            // draw indicator hand

            ctx.set_stroke_style(&JsValue::from_str("#8d8bb0"));

            let x = ROTARY_CENTER_X + (ROTARY_RADIUS - ROTARY_HAND_WIDTH / 2.0) * value_angle.cos();
            let y = ROTARY_CENTER_Y + (ROTARY_RADIUS - ROTARY_HAND_WIDTH / 2.0) * value_angle.sin();

            ctx.begin_path();
            ctx.set_line_width(ROTARY_HAND_WIDTH);
            ctx.move_to(ROTARY_CENTER_X, ROTARY_CENTER_Y);
            ctx.line_to(x, y);
            ctx.stroke();

            // draw rounded ends on indicator hand

            ctx.set_fill_style(&JsValue::from_str("#8d8bb0"));

            ctx.begin_path();
            let _ = ctx.ellipse(
                ROTARY_CENTER_X,
                ROTARY_CENTER_Y,
                ROTARY_HAND_WIDTH / 2.0,
                ROTARY_HAND_WIDTH / 2.0,
                0.0,
                0.0,
                f64::consts::PI * 2.0,
            );
            ctx.fill();

            ctx.begin_path();
            let _ = ctx.ellipse(
                x,
                y,
                ROTARY_HAND_WIDTH / 2.0,
                ROTARY_HAND_WIDTH / 2.0,
                0.0,
                0.0,
                f64::consts::PI * 2.0,
            );
            ctx.fill();
        }
    }
}
