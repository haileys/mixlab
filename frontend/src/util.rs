use std::num::NonZeroUsize;

use yew::Callback;
use web_sys::Event;

pub fn stop_propagation<In>() -> Callback<In>
where
    In: AsRef<Event>
{
    Callback::from(|_: In| {
        // no-op, yew already stops propagation for us
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

pub fn websocket_origin() -> String {
    let location = web_sys::window().unwrap().location();
    let proto = location.protocol().unwrap();
    let host = location.host().unwrap();

    let proto = match proto.as_str() {
        "https" => "wss",
        _ => "ws",
    };

    format!("{}://{}", proto, host)
}
