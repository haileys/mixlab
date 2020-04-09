use yew::{Component, ComponentLink, Callback};
use yew::events::ChangeData;
use web_sys::Event;

pub fn stop_propagation<In>() -> Callback<In>
where
    In: AsRef<Event>
{
    Callback::from(|ev: In| {
        ev.as_ref().stop_propagation();
    })
}

pub fn prevent_default<In>() -> Callback<In>
where
    In: AsRef<Event>
{
    Callback::from(|ev: In| {
        ev.as_ref().stop_propagation();
        ev.as_ref().prevent_default();
    })
}

pub fn callback_ex<Comp, F, In, M>(link: &ComponentLink<Comp>, f: F) -> Callback<In>
where
    Comp: Component,
    M: Into<Comp::Message>,
    F: Fn(In) -> M + 'static,
    In: AsRef<Event>
{
    link.callback(move |ev: In| {
        ev.as_ref().stop_propagation();
        ev.as_ref().prevent_default();
        f(ev)
    })
}

pub fn clamp<T: PartialOrd>(min: T, max: T, val: T) -> T {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

pub fn extract_callback_float_value(event: ChangeData) -> Option<f64> {
    match event {
        ChangeData::Value(float_str) => float_str.parse().ok(),
        _ => None
    }
}
