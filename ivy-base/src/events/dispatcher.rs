use std::sync::mpsc;

use downcast_rs::{impl_downcast, Downcast};
use parking_lot::Mutex;

use super::Event;

pub trait AnyEventDispatcher: 'static + Send + Sync + Downcast {}
impl_downcast!(AnyEventDispatcher);

pub trait AnyEventSender: 'static + Send + Sync + Downcast {}
impl_downcast!(AnyEventSender);

/// Handles event dispatching for a single type of event
pub struct EventDispatcher<T: Event> {
    subscribers: Mutex<Vec<Subscriber<T>>>,
    pub blocked: bool,
}

impl<T> EventDispatcher<T>
where
    T: Event,
{
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            blocked: false,
        }
    }

    /// Sends an event to all subscribed subscriber. Event is cloned for each registered subscriber. Requires mutable access to cleanup no longer active subscribers.
    pub fn send(&self, event: T) {
        if self.blocked {
            return;
        }

        let mut subscribers = self.subscribers.lock();
        if subscribers.len() == 1 {
            subscribers[0].send(event);
        } else {
            subscribers.retain(|subscriber| {
                if (subscriber.filter)(&event) {
                    subscriber.send(event.clone())
                } else {
                    true
                }
            });
        }
    }

    /// Subscribes to events using sender to send events. The subscriber is automatically cleaned
    /// up when the receiving end is dropped.
    pub fn subscribe<S>(&self, sender: S, filter: fn(&T) -> bool)
    where
        S: 'static + EventSender<T> + Send,
    {
        self.subscribers
            .lock()
            .push(Subscriber::new(sender, filter));
    }
}

impl<T: Event> AnyEventDispatcher for EventDispatcher<T> {}

struct Subscriber<T> {
    sender: Box<dyn EventSender<T> + Send>,
    filter: fn(&T) -> bool,
}

impl<T: Event> Subscriber<T> {
    pub fn new<S>(sender: S, filter: fn(&T) -> bool) -> Self
    where
        S: 'static + EventSender<T> + Send,
    {
        Self {
            sender: Box::new(sender),
            filter,
        }
    }
    pub fn send(&self, event: T) -> bool {
        self.sender.send(event)
    }
}

/// Describes a type which can send events. Implemented for mpsc::channel and crossbeam channel.
pub trait EventSender<T>: 'static + Send {
    /// Send an event. Returns true if receiver is still alive.
    fn send(&self, event: T) -> bool;
}

impl<T: Event> EventSender<T> for mpsc::Sender<T> {
    fn send(&self, event: T) -> bool {
        self.send(event).is_ok()
    }
}

#[cfg(feature = "crossbeam-channel")]
impl<T: Event> EventSender<T> for crossbeam_channel::Sender<T> {
    fn send(&self, event: T) -> bool {
        self.send(event).is_ok()
    }
}

impl<T: Event> EventSender<T> for flume::Sender<T> {
    fn send(&self, event: T) -> bool {
        self.send(event).is_ok()
    }
}

pub fn new_event_dispatcher<T: Event>() -> Box<dyn AnyEventDispatcher> {
    let dispatcher: EventDispatcher<T> = EventDispatcher::new();
    Box::new(dispatcher)
}

pub struct ConcreteSender<T> {
    inner: Mutex<Box<dyn EventSender<T>>>,
}

impl<T> ConcreteSender<T> {
    pub fn new<S: EventSender<T>>(sender: S) -> Self {
        Self {
            inner: Mutex::new(Box::new(sender)),
        }
    }
}

impl<T: Event> EventSender<T> for ConcreteSender<T> {
    fn send(&self, event: T) -> bool {
        self.inner.lock().send(event)
    }
}

impl<T: Event> AnyEventSender for ConcreteSender<T> {}
