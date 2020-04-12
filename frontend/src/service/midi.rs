use std::cell::RefCell;
use std::collections::{HashMap, BTreeMap};
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::usize;

use gloo_events::EventListener;
use js_sys::Map;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{MidiInput, MidiMessageEvent};
use yew::Callback;

use crate::util::Sequence;

struct MidiBroker {
    inputs: HashMap<MidiInputId, MidiInput>,
    listeners: HashMap<MidiInputId, EventListener>,
    configuring: Option<ConfigureKind>,
    id_seq: Sequence,
    range_subscribers: BTreeMap<(MidiRangeId, SubscriptionId), Callback<u8>>,
}

#[derive(Clone)]
pub struct MidiBrokerRef(Rc<RefCell<MidiBroker>>);

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct MidiRangeId(MidiInputId, u8);

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct MidiNoteId(MidiInputId, u8);

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
struct SubscriptionId(NonZeroUsize);

impl SubscriptionId {
    fn min() -> SubscriptionId {
        SubscriptionId(NonZeroUsize::new(1).unwrap())
    }

    fn max() -> SubscriptionId {
        SubscriptionId(NonZeroUsize::new(usize::MAX).unwrap())
    }
}

type MidiInputId = Rc<String>;

thread_local! {
    static BROKER: MidiBrokerRef = MidiBroker::new();
}

pub fn broker() -> MidiBrokerRef {
    BROKER.with(|cell| cell.clone())
}

impl MidiBrokerRef {
    pub fn configure_range(&self, callback: Callback<Option<(MidiRangeId, u8)>>) {
        self.0.borrow_mut().configure(ConfigureKind::Range(callback));
    }

    pub fn subscribe_range(&self, range_id: MidiRangeId, callback: Callback<u8>) -> RangeSubscription {
        let key = {
            let mut broker = self.0.borrow_mut();
            let subscription_id = SubscriptionId(broker.id_seq.next());
            let key = (range_id, subscription_id);
            broker.range_subscribers.insert(key.clone(), callback);
            key
        };

        RangeSubscription {
            broker: self.clone(),
            key,
        }
    }

    fn on_message(&self, input_id: MidiInputId, event: &MidiMessageEvent) {
        let data = event.data().expect("MidiMessageEvent::data");

        // MIDI controller (range) change message
        if data.len() == 3 && (data[0] & 0xf0) == 0xb0 {
            let range_id = MidiRangeId(input_id, data[1] & 0x7f);
            let value = data[2] & 0x7f;

            let min_key = (range_id.clone(), SubscriptionId::min());
            let max_key = (range_id.clone(), SubscriptionId::max());

            let mut subscribers = Vec::new();
            let mut configuring = None;

            {
                let mut broker = self.0.borrow_mut();

                for (_, callback) in broker.range_subscribers.range(min_key..=max_key) {
                    subscribers.push(callback.clone());
                }

                if let Some(ConfigureKind::Range(callback)) = &broker.configuring {
                    configuring = Some(callback.clone());
                    broker.configuring = None;
                }
            }

            for callback in subscribers {
                callback.emit(value);
            }

            if let Some(callback) = configuring {
                callback.emit(Some((range_id, value)));
            }
        }
    }
}

impl MidiBroker {
    pub fn new() -> MidiBrokerRef {
        let broker = MidiBrokerRef(Rc::new(RefCell::new(MidiBroker {
            inputs: HashMap::new(),
            listeners: HashMap::new(),
            configuring: None,
            id_seq: Sequence::new(),
            range_subscribers: BTreeMap::new(),
        })));

        wasm_bindgen_futures::spawn_local({
            let broker = broker.clone();
            async move {
                setup(broker).await
                    .expect("setup");
            }
        });

        broker
    }

    fn configure(&mut self, configure: ConfigureKind) {
        if let Some(previous) = self.configuring.take() {
            match previous {
                ConfigureKind::Range(cb) => cb.emit(None),
            }
        }

        self.configuring = Some(configure);
    }
}

pub struct RangeSubscription {
    broker: MidiBrokerRef,
    key: (MidiRangeId, SubscriptionId),
}

impl Drop for RangeSubscription {
    fn drop(&mut self) {
        self.broker.0.borrow_mut().range_subscribers.remove(&self.key);
    }
}

enum ConfigureKind {
    Range(Callback<Option<(MidiRangeId, u8)>>),
    // Note(Callback<Option<MidiNoteId>>),
}

async fn request_midi_access() -> Result<web_sys::MidiAccess, JsValue> {
    Ok(JsFuture::from(web_sys::window()
        .expect("web_sys::window")
        .navigator()
        .request_midi_access()
        .expect("navigator.request_midi_access"))
        .await?
        .dyn_into::<web_sys::MidiAccess>()?)
}

async fn setup_input(broker: MidiBrokerRef, input: MidiInput) -> Result<(), JsValue> {
    let input_id = Rc::new(input.id());

    let event_listener = EventListener::new(&input, "midimessage", {
        let input_id = input_id.clone();
        let broker = broker.clone();
        move |ev| {
            let message = ev.dyn_ref::<MidiMessageEvent>()
                .expect("dyn_into MidiMessageEvent");

            broker.on_message(input_id.clone(), message);
        }
    });

    let mut broker = broker.0.borrow_mut();
    broker.inputs.insert(input_id.clone(), input);
    broker.listeners.insert(input_id.clone(), event_listener);

    Ok(())
}

async fn setup(broker: MidiBrokerRef) -> Result<(), JsValue> {
    let midi = request_midi_access().await?;

    let inputs = midi.inputs()
        // MidiInputMap is not instanceof a Map, but is defined to adhere to
        // the same interface for read-only methods:
        .unchecked_into::<Map>();

    let inputs = js_sys::try_iter(&inputs.values())
        .expect("inputs try_iter")
        .expect("inputs try_iter");

    for input in inputs {
        let input = input?.dyn_into::<MidiInput>()
            .expect("dyn_into MidiInput");

        setup_input(broker.clone(), input).await?;
    }

    Ok(())
}