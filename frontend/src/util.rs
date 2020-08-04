use std::num::NonZeroUsize;

use wasm_bindgen::JsCast;
use web_sys::{Event, Element, HtmlElement};
use yew::Callback;

use mixlab_protocol::Coords;

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
        ev.as_ref().prevent_default();
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

#[derive(Debug)]
pub struct Sequence(usize);

impl Sequence {
    pub fn new() -> Self {
        Sequence(0)
    }

    /// Returns the last sequence number generated:
    pub fn last(&self) -> Option<NonZeroUsize> {
        NonZeroUsize::new(self.0)
    }

    /// Generates a new sequence number
    pub fn next(&mut self) -> NonZeroUsize {
        self.0 += 1;
        NonZeroUsize::new(self.0).unwrap()
    }
}

pub fn origin() -> String {
    let location = web_sys::window().unwrap().location();
    let proto = location.protocol().unwrap();
    let host = location.host().unwrap();
    format!("{}//{}", proto, host)
}

pub fn websocket_origin() -> String {
    let location = web_sys::window().unwrap().location();
    let proto = location.protocol().unwrap();
    let host = location.host().unwrap();

    let proto = match proto.as_str() {
        "https:" => "wss:",
        _ => "ws:",
    };

    format!("{}//{}", proto, host)
}

fn html_element_parent(mut element: Element) -> Option<HtmlElement> {
    loop {
        match element.dyn_ref::<HtmlElement>() {
            Some(html_element) => { return Some(html_element.clone()); }
            None => {
                match element.parent_element() {
                    Some(parent) => { element = parent; }
                    None => { return None; }
                }
            }
        }
    }
}

pub fn offset_coords_in(container: HtmlElement, element: Element) -> Option<Coords> {
    let mut element = html_element_parent(element)?;
    let mut coords = Coords { x: 0, y: 0 };

    while element != container {
        coords.x += element.offset_left();
        coords.y += element.offset_top();

        match element.offset_parent() {
            Some(parent) => { element = parent.dyn_into::<HtmlElement>().unwrap(); }
            None => { return None; }
        }
    }

    Some(coords)
}
