use std::{any::TypeId, collections::HashMap, sync::mpsc};

use downcast_rs::{impl_downcast, Downcast};

pub struct Events {
    dispatchers: HashMap<TypeId, Box<dyn AnyEventDispatcher>>,
}

impl Events {
    pub fn new() -> Events {
        Self {
            dispatchers: HashMap::new(),
        }
    }

    /// Sends an event of type T to all subscribed listeners.
    /// If no dispatcher exists for event T, a new one will be created.
    pub fn send<T: 'static + Clone + Send + Sync>(&mut self, event: T) {
        self.dispatchers
            .entry(TypeId::of::<T>())
            .or_insert_with(new_event_dispatcher::<T>)
            .downcast_mut::<EventDispatcher<T>>()
            .map(|dispatcher| dispatcher.send(event));
    }

    pub fn subscribe<S, T: 'static + Clone + Send + Sync>(&mut self, sender: S)
    where
        S: 'static + EventSender<T> + Send + Sync,
    {
        self.dispatchers
            .entry(TypeId::of::<T>())
            .or_insert_with(new_event_dispatcher::<T>)
            .downcast_mut::<EventDispatcher<T>>()
            .map(|dispatcher| dispatcher.subscribe(sender));
    }
}

trait AnyEventDispatcher: 'static + Send + Sync + Downcast {}
impl_downcast!(AnyEventDispatcher);

/// Handles event dispatching for a single type of event
pub struct EventDispatcher<T> {
    subscribers: Vec<Subscriber<T>>,
}

impl<T> EventDispatcher<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Sends an event to all subscribed subscriber. Event is cloned for each registered subscriber. Requires mutable access to cleanup no longer active subscribers.
    pub fn send(&mut self, event: T) {
        if self.subscribers.len() == 1 {
            self.subscribers[0].send(event);
        } else {
            self.subscribers
                .retain(|subscriber| subscriber.send(event.clone()));
        }
    }

    /// Subscribes to events using sender to send events. The subscriber is automatically cleaned
    /// up when the receiving end is dropped.
    pub fn subscribe<S>(&mut self, sender: S)
    where
        S: 'static + EventSender<T> + Send + Sync,
    {
        self.subscribers.push(Subscriber::new(sender));
    }
}

impl<T: 'static + Send + Sync + Clone> AnyEventDispatcher for EventDispatcher<T> {}

struct Subscriber<T> {
    sender: Box<dyn EventSender<T> + Send + Sync>,
}

impl<T> Subscriber<T> {
    pub fn new<S>(sender: S) -> Self
    where
        S: 'static + EventSender<T> + Send + Sync,
    {
        Self {
            sender: Box::new(sender),
        }
    }
    pub fn send(&self, event: T) -> bool {
        self.sender.send(event)
    }
}

/// Describes a type which can send events. Implemented for mpsc::channel and crossbeam channel.
pub trait EventSender<T> {
    /// Send an event. Returns true if receiver is still alive.
    fn send(&self, event: T) -> bool;
}

impl<T> EventSender<T> for mpsc::Sender<T> {
    fn send(&self, event: T) -> bool {
        match self.send(event) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

#[cfg(feature = "crossbeam-channel")]
impl<T> EventSender<T> for crossbeam_channel::Sender<T> {
    fn send(&self, event: T) -> bool {
        match self.send(event) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

impl<T> EventSender<T> for flume::Sender<T> {
    fn send(&self, event: T) -> bool {
        match self.send(event) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

fn new_event_dispatcher<T: 'static + Clone + Send + Sync>() -> Box<dyn AnyEventDispatcher> {
    let dispatcher: EventDispatcher<T> = EventDispatcher::new();
    Box::new(dispatcher)
}
