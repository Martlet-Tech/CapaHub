use crate::event::Event;
use std::sync::{Arc, RwLock};

type EventHandler = Arc<dyn Fn(Arc<dyn Event>) + Send + Sync>;

pub struct EventBus {
    subscribers: RwLock<Vec<(String, EventHandler)>>,
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            subscribers: RwLock::new(Vec::new()),
        }
    }

    pub fn subscribe(&self, event_type: &str, handler: EventHandler) {
        let mut subs = self.subscribers.write().unwrap();
        subs.push((event_type.to_string(), handler));
    }

    pub fn publish(&self, event: Arc<dyn Event>) {
        let event_type = event.event_type();
        let subs = self.subscribers.read().unwrap();
        for (et, handler) in subs.iter() {
            if *et == event_type {
                handler(event.clone());
            }
        }
    }
}
