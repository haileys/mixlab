use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::{self, Debug};
use std::num::NonZeroUsize;
use std::rc::Rc;

use yew::Callback;

use crate::util::Sequence;

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
struct HandleId(NonZeroUsize);

#[derive(Debug)]
pub struct Notify<T> {
    id_seq: RefCell<Sequence>,
    value: RefCell<Option<T>>,
    map: NotifyMap<T>,
}

impl<T: Clone + 'static> Notify<T> {
    pub fn new() -> Self {
        Notify {
            id_seq: RefCell::new(Sequence::new()),
            value: RefCell::new(None),
            map: NotifyMap(Rc::new(RefCell::new(BTreeMap::new()))),
        }
    }

    pub fn subscribe(&self, f: Callback<T>) -> Handle {
        // send existing value if exists straight away
        if let Some(val) = self.value.borrow().as_ref().cloned() {
            f.emit(val);
        }

        let id = HandleId(self.id_seq.borrow_mut().next());

        self.map.0.borrow_mut().insert(id, f);

        Handle {
            id,
            dereg: Box::new(self.map.clone()) as Box<dyn Deregister>,
        }
    }

    pub fn broadcast(&self, value: T) {
        *self.value.borrow_mut() = Some(value.clone());

        for callback in self.map.0.borrow().values() {
            callback.emit(value.clone());
        }
    }
}

#[derive(Debug, Clone)]
struct NotifyMap<T>(Rc<RefCell<BTreeMap<HandleId, Callback<T>>>>);

trait Deregister {
    fn deregister(&self, handle: HandleId);
}

impl<T> Deregister for NotifyMap<T> {
    fn deregister(&self, handle: HandleId) {
        self.0.borrow_mut().remove(&handle);
    }
}

pub struct Handle {
    id: HandleId,
    dereg: Box<dyn Deregister>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.dereg.deregister(self.id)
    }
}

impl Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Handle({:?})", self.id)
    }
}
