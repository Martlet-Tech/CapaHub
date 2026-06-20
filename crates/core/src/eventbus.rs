use crate::event::Event;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub type SubscriptionId = u64;

type EventHandler = Arc<dyn Fn(Arc<dyn Event>) + Send + Sync>;

struct Subscriber {
    id: SubscriptionId,
    event_type: String,
    handler: EventHandler,
}

pub struct EventBus {
    next_id: AtomicU64,
    subscribers: RwLock<Vec<Subscriber>>,
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            next_id: AtomicU64::new(1),
            subscribers: RwLock::new(Vec::new()),
        }
    }

    pub fn subscribe(&self, event_type: &str, handler: EventHandler) -> SubscriptionId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut subs = self.subscribers.write().unwrap();
        subs.push(Subscriber {
            id,
            event_type: event_type.to_string(),
            handler,
        });
        id
    }

    pub fn unsubscribe(&self, id: SubscriptionId) {
        let mut subs = self.subscribers.write().unwrap();
        subs.retain(|s| s.id != id);
    }

    pub fn publish(&self, event: Arc<dyn Event>) {
        let event_type = event.event_type();
        let subs = self.subscribers.read().unwrap();
        for sub in subs.iter() {
            if sub.event_type == event_type {
                (sub.handler)(event.clone());
            }
        }
    }
}
