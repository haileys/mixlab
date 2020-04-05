use itertools::{Itertools, Either};
use plotters::prelude::*;
use web_sys::HtmlCanvasElement;
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

use mixlab_protocol::{ModuleId, PlotterIndication};

#[derive(Properties, Clone, Debug)]
pub struct PlotterProps {
    pub id: ModuleId,
    pub indication: PlotterIndication,
    pub height: usize,
    pub width: usize,
}

pub struct Plotter {
    props: PlotterProps,
    canvas_1: NodeRef,
    canvas_2: NodeRef,
}

impl Component for Plotter {
    type Properties = PlotterProps;
    type Message = ();

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Plotter {
            props,
            canvas_1: NodeRef::default(),
            canvas_2: NodeRef::default(),
        }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn mounted(&mut self) -> ShouldRender {
        true
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;

        if let Some(input) = &self.props.indication.inputs[0] {
            let (channel_1, channel_2): (Vec<f32>, Vec<f32>) = input.into_iter().enumerate().partition_map(|(i, sample)| {
                if i % 2 == 0 {
                    Either::Left(sample)
                } else {
                    Either::Right(sample)
                }
            });

            if let Some(canvas) = self.canvas_1.cast::<HtmlCanvasElement>() {
                plot(canvas, &channel_1);

            }
            if let Some(canvas) = self.canvas_2.cast::<HtmlCanvasElement>() {
                plot(canvas, &channel_2);
            }
        }

        true
    }

    fn view(&self) -> Html {
        html! {
            <>
                <canvas ref={self.canvas_1.clone()} width={self.props.width} height={self.props.height}></canvas>
                <canvas ref={self.canvas_2.clone()} width={self.props.width} height={self.props.height}></canvas>
            </>
        }
    }
}

fn plot(canvas: HtmlCanvasElement, channel: &[f32]) {
    let backend = CanvasBackend::with_canvas_object(canvas).unwrap();
    let root = backend.into_drawing_area();
    root.fill(&WHITE).unwrap();

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_ranged(0f32..440., -1f32..1f32).unwrap();
    chart.configure_mesh().x_labels(3).y_labels(3).draw().unwrap();

    let series = channel
        .iter()
        .enumerate()
        .map(|(x, y)| (x as f32, *y))
        .collect::<Vec<(f32, f32)>>();
    chart.draw_series(LineSeries::new(series, &RED)).unwrap();

    root.present().unwrap();
}
