use std::sync::mpsc;

use downcast_rs::{impl_downcast, Downcast};
use parking_lot::Mutex;

use super::Event;

pub trait AnyEventDispatcher: 'static + Send + Sync + Downcast {
    fn cleanup(&mut self);
}

impl_downcast!(AnyEventDispatcher);

pub trait AnyEventSender: 'static + Send + Sync + Downcast {}
impl_downcast!(AnyEventSender);

/// Handles event dispatching for a single type of event
pub struct EventDispatcher<T: Event> {
    subscribers: Vec<Subscriber<T>>,
    pub blocked: bool,
}

impl<T> EventDispatcher<T>
where
    T: Event,
{
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            blocked: false,
        }
    }

    /// Sends an event to all subscribed subscriber. Event is cloned for each registered subscriber. Requires mutable access to cleanup no longer active subscribers.
    pub fn send(&self, event: T) {
        if self.blocked {
            return;
        }

        for subscriber in &self.subscribers {
            if (subscriber.filter)(&event) {
                subscriber.send(event.clone());
            }
        }
    }

    /// Subscribes to events using sender to send events. The subscriber is automatically cleaned
    /// up when the receiving end is dropped.
    pub fn subscribe<S>(&mut self, sender: S, filter: fn(&T) -> bool)
    where
        S: 'static + EventSender<T> + Send,
    {
        self.subscribers.push(Subscriber::new(sender, filter));
    }
}

impl<T> Default for EventDispatcher<T>
where
    T: Event,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Event> AnyEventDispatcher for EventDispatcher<T> {
    fn cleanup(&mut self) {
        self.subscribers.retain(|val| !val.sender.is_disconnected())
    }
}

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
    pub fn send(&self, event: T) {
        self.sender.send(event)
    }
}

/// Describes a type which can send events. Implemented for mpsc::channel and crossbeam channel.
pub trait EventSender<T>: 'static + Send + Sync {
    /// Send an event
    fn send(&self, event: T);
    /// Returns true if the sender has been disconnected
    fn is_disconnected(&self) -> bool;
}

/// Wrapper for thread safe sender
pub struct MpscSender<T> {
    inner: Mutex<(bool, mpsc::Sender<T>)>,
}

impl<T> From<mpsc::Sender<T>> for MpscSender<T> {
    fn from(val: mpsc::Sender<T>) -> Self {
        Self::new(val)
    }
}

impl<T> MpscSender<T> {
    pub fn new(inner: mpsc::Sender<T>) -> Self {
        Self {
            inner: Mutex::new((false, inner)),
        }
    }
}

impl<T: Event> EventSender<T> for MpscSender<T> {
    fn send(&self, event: T) {
        let mut inner = self.inner.lock();
        match inner.1.send(event) {
            Ok(_) => {}
            Err(_) => inner.0 = true,
        }
    }

    fn is_disconnected(&self) -> bool {
        // TODO
        self.inner.lock().0
        // self.inner.is_disconnected()
    }
}

#[cfg(feature = "crossbeam-channel")]
impl<T: Event> EventSender<T> for crossbeam_channel::Sender<T> {
    fn send(&self, event: T) -> bool {
        let _ = self.send(event);
    }

    fn is_disconnected(&self) -> bool {
        self.is_disconnected
    }
}

impl<T: Event> EventSender<T> for flume::Sender<T> {
    fn send(&self, event: T) {
        let _ = self.send(event);
    }

    fn is_disconnected(&self) -> bool {
        self.is_disconnected()
    }
}

pub fn new_event_dispatcher<T: Event>() -> Box<dyn AnyEventDispatcher> {
    let dispatcher: EventDispatcher<T> = EventDispatcher::new();
    Box::new(dispatcher)
}

pub struct ConcreteSender<T> {
    inner: Box<dyn EventSender<T>>,
}

impl<T> ConcreteSender<T> {
    pub fn new<S: EventSender<T>>(sender: S) -> Self {
        Self {
            inner: Box::new(sender),
        }
    }
}

impl<T: Event> EventSender<T> for ConcreteSender<T> {
    fn send(&self, event: T) {
        self.inner.send(event)
    }

    fn is_disconnected(&self) -> bool {
        self.inner.is_disconnected()
    }
}

impl<T: Event> AnyEventSender for ConcreteSender<T> {}
