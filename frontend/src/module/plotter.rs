use plotters::prelude::*;
use web_sys::HtmlCanvasElement;
use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties, NodeRef};

use mixlab_protocol::{ModuleId, PlotterIndication};

#[derive(Properties, Clone, Debug)]
pub struct PlotterProps {
    pub id: ModuleId,
    pub indication: PlotterIndication,
}

struct Plot {
    canvas: NodeRef,
}

pub struct Plotter {
    props: PlotterProps,
    plots: Vec<Plot>,
}

impl Component for Plotter {
    type Properties = PlotterProps;
    type Message = ();

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Plotter {
            props,
            plots: vec![
                Plot { canvas: NodeRef::default() },
                Plot { canvas: NodeRef::default() },
            ],
        }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;

        for (index, input) in self.props.indication.inputs.iter().enumerate() {
            if let Some(plot) = self.plots.get(index) {
                if let Some(canvas) = plot.canvas.cast::<HtmlCanvasElement>() {
                    render_plot(canvas, input);
                }
            }
        }

        true
    }

    fn view(&self) -> Html {
        let chart_height = 150;
        let chart_width = 300;

        html! {
            { for self.plots.iter().map(|plot| {
                html! { <canvas ref={plot.canvas.clone()} width={chart_width} height={chart_height} /> }
            })}
        }
    }
}

fn render_plot(canvas: HtmlCanvasElement, channel: &[f32]) {
    let backend = CanvasBackend::with_canvas_object(canvas).unwrap();
    let root = backend.into_drawing_area();
    root.fill(&WHITE).unwrap();

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(20)
        .y_label_area_size(45)
        .build_ranged(0f32..440., -1.01f32..1.1f32).unwrap();
    chart.configure_mesh()
        .x_labels(4)
        .x_label_formatter(&|x| format!("{:.1}ms", *x as f64 / 44.1))
        .y_labels(5)
        .y_desc("Amplitude")
        .draw()
        .unwrap();

    let series = channel
        .iter()
        .enumerate()
        .map(|(x, y)| (x as f32, *y))
        .collect::<Vec<(f32, f32)>>();
    chart.draw_series(LineSeries::new(series, &RED)).unwrap();

    root.present().unwrap();
}
