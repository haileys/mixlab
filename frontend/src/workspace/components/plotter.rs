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
    canvas: NodeRef,
}

impl Component for Plotter {
    type Properties = PlotterProps;
    type Message = ();

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Plotter {
            props,
            canvas: NodeRef::default(),
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

        use plotters::prelude::*;

        if let Some(canvas) = self.canvas.cast::<HtmlCanvasElement>() {
            let backend = CanvasBackend::with_canvas_object(canvas).unwrap();
            let root = backend.into_drawing_area();
            root.fill(&WHITE).unwrap();

            let mut chart = ChartBuilder::on(&root)
                .x_label_area_size(30)
                .y_label_area_size(30)
                .build_ranged(0f32..440., -1f32..1f32).unwrap();
            chart.configure_mesh().x_labels(3).y_labels(3).draw().unwrap();

            let colors = [BLUE, GREEN];

            for (i, input) in self.props.indication.inputs.iter().enumerate() {
                if let Some(input) = input {
                    let series = input
                        .iter()
                        .enumerate()
                        .map(|(x, y)| (x as f32, *y))
                        .collect::<Vec<(f32, f32)>>();
                    chart.draw_series(LineSeries::new(series, &colors[i])).unwrap();
                }
            }

            root.present().unwrap();
        }

        true
    }

    fn view(&self) -> Html {
        html! { <canvas ref={self.canvas.clone()} width={self.props.width} height={self.props.height}></canvas> }
    }
}
