use yew::{Component, ComponentLink, Callback};
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

pub struct Sequence(usize);

impl Sequence {
    pub fn new() -> Self {
        Sequence(0)
    }

    /// Returns the last sequence number generated:
    pub fn last(&self) -> Option<usize> {
        if self.0 == 0 {
            None
        } else {
            Some(self.0)
        }
    }

    /// Generates a new sequence number
    pub fn next(&mut self) -> usize {
        self.0 += 1;
        self.0
    }
}
