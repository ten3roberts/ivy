use std::{any::TypeId, collections::HashMap, sync::mpsc};

use downcast_rs::{impl_downcast, Downcast};

/// Manages event broadcasting for different types of events.
/// Sending an event will send a clone of the event to all subscribed listeners.
///
/// The event listeners can be anything implementing `EventSender`. Implemented by `std::sync::mpsc::Sender`,
/// `flume::Sender`, `crossbeam_channel::Sender`.
///
/// # Example
/// ```
/// use ivy_core::Events;
/// use std::sync::mpsc;
/// let mut events = Events::new();
///
/// let (tx1, rx1) = mpsc::channel::<&'static str>();
/// events.subscribe(tx1);
///
/// let (tx2, rx2) = mpsc::channel::<&'static str>();
/// events.subscribe(tx2);
///
/// events.send("Hello");
///
/// if let Ok(e) = rx1.try_recv() {
///     println!("1 Received: {}", e);
/// }
///
/// if let Ok(e) = rx2.try_recv() {
///     println!("2 Received: {}", e);
/// }
/// ```
pub struct Events {
    dispatchers: HashMap<TypeId, Box<dyn AnyEventDispatcher>>,
}

impl Events {
    pub fn new() -> Events {
        Self {
            dispatchers: HashMap::new(),
        }
    }

    /// Sends an event of type `T` to all subscribed listeners.
    /// If no dispatcher exists for event `T`, a new one will be created.
    pub fn send<T: Event>(&mut self, event: T) {
        let dispatcher = self
            .dispatchers
            .entry(TypeId::of::<T>())
            .or_insert_with(new_event_dispatcher::<T>)
            .downcast_mut::<EventDispatcher<T>>()
            .expect("Failed to downcast");

        dispatcher.send(event)
    }

    pub fn subscribe<S, T: Event>(&mut self, sender: S)
    where
        S: 'static + EventSender<T> + Send,
    {
        if let Some(dispatcher) = self
            .dispatchers
            .entry(TypeId::of::<T>())
            .or_insert_with(new_event_dispatcher::<T>)
            .downcast_mut::<EventDispatcher<T>>()
        {
            dispatcher.subscribe(sender)
        }
    }
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

// Blanket type for events.
pub trait Event: 'static + Clone + Send {}
impl<T: 'static + Clone + Send> Event for T {}

trait AnyEventDispatcher: 'static + Send + Downcast {}
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
        S: 'static + EventSender<T> + Send,
    {
        self.subscribers.push(Subscriber::new(sender));
    }
}

impl<T: 'static + Send + Clone> AnyEventDispatcher for EventDispatcher<T> {}

struct Subscriber<T> {
    sender: Box<dyn EventSender<T> + Send>,
}

impl<T> Subscriber<T> {
    pub fn new<S>(sender: S) -> Self
    where
        S: 'static + EventSender<T> + Send,
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
        self.send(event).is_ok()
    }
}

#[cfg(feature = "crossbeam-channel")]
impl<T> EventSender<T> for crossbeam_channel::Sender<T> {
    fn send(&self, event: T) -> bool {
        self.send(event).is_ok()
    }
}

impl<T> EventSender<T> for flume::Sender<T> {
    fn send(&self, event: T) -> bool {
        self.send(event).is_ok()
    }
}

fn new_event_dispatcher<T: 'static + Clone + Send>() -> Box<dyn AnyEventDispatcher> {
    let dispatcher: EventDispatcher<T> = EventDispatcher::new();
    Box::new(dispatcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn event_broadcast() {
        let mut events = Events::new();

        let (tx1, rx1) = mpsc::channel::<&'static str>();
        events.subscribe(tx1);

        let (tx2, rx2) = mpsc::channel::<&'static str>();
        events.subscribe(tx2);

        events.send("Hello");

        if let Ok(e) = rx1.try_recv() {
            assert_eq!(e, "Hello")
        }

        if let Ok(e) = rx2.try_recv() {
            assert_eq!(e, "Hello")
        }
    }
}
