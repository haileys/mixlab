use std::cell::RefCell;
use std::rc::Rc;

use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};

use mixlab_protocol::{PerformanceInfo, PerformanceAccount, TemporalWarningStatus};

use crate::State;

pub struct Sidebar {
    props: SidebarProps,
}

#[derive(Properties, Clone, Debug)]
pub struct SidebarProps {
    pub state: Rc<RefCell<State>>,
    pub performance_info: Option<Rc<PerformanceInfo>>,
}

impl Component for Sidebar {
    type Properties = SidebarProps;
    type Message = ();

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Sidebar {
            props,
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, _: Self::Message) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="sidebar">
                <div class="sidebar-title">{"Mixlab"}</div>
                {self.view_perf_info()}
            </div>
        }
    }
}

impl Sidebar {
    fn view_perf_info(&self) -> Html {
        if let Some(perf_info) = &self.props.performance_info {
            let state = self.props.state.borrow();

            let realtime_status_class = if perf_info.realtime {
                "status-light status-light-green-active"
            } else {
                "status-light"
            };

            let lag_status_class = match perf_info.lag {
                None => "status-light",
                Some(TemporalWarningStatus::Active) => "status-light status-light-red-active",
                Some(TemporalWarningStatus::Recent) => "status-light status-light-red",
            };

            let tick_budget = perf_info.tick_budget.0 as f64;

            let total_tick_time: u64 = perf_info.accounts.iter()
                .map(|(_, metric)| metric.last.0)
                .sum();

            let total_tick_percent = (total_tick_time as f64 / tick_budget) * 100.0;

            let mut sorted_accounts = perf_info.accounts.clone();
            sorted_accounts.sort_by(|(_, a), (_, b)| b.last.cmp(&a.last));

            html! {
                <div class="perf-info">
                    <div class="status-light-bar">
                        <div class={realtime_status_class}>{"REALTIME"}</div>
                        <div class={lag_status_class}>{"LAG"}</div>
                    </div>
                    <div class="perf-info-tick-util">
                        {format!("{:2.1}%", total_tick_percent)}
                    </div>
                    <table class="perf-info-accounts-table">
                        { for sorted_accounts.iter().map(|(account, metric)| {
                            let percent = (metric.last.0 as f64 / tick_budget) * 100.0;

                            html! {
                                <tr>
                                    { match account {
                                        PerformanceAccount::Engine => {
                                            html! { <td class="perf-info-account perf-info-account-engine">{"Engine"}</td> }
                                        }
                                        PerformanceAccount::Module(id) => {
                                            let name = state.modules.get(id).map(|module| {
                                                format!("{:?}", module).chars().take_while(|c| c.is_alphanumeric()).collect::<String>()
                                            }).unwrap_or("-".to_owned());

                                            html! { <td class="perf-info-account perf-info-account-module">{name}</td> }
                                        }
                                    } }
                                    <td class="perf-info-metric">{format!("{:2.1}%", percent)}</td>
                                </tr>
                            }
                        }) }
                    </table>
                </div>
            }
        } else {
            html! {}
        }
    }
}
