use std::{any::TypeId, collections::HashMap, sync::mpsc};

use downcast_rs::{impl_downcast, Downcast};
use hecs::Component;
use parking_lot::Mutex;

/// Manages event broadcasting for different types of events.
/// Sending an event will send a clone of the event to all subscribed listeners.
///
/// The event listeners can be anything implementing `EventSender`. Implemented by `std::sync::mpsc::Sender`,
/// `flume::Sender`, `crossbeam_channel::Sender`.
///
/// # Example
/// ```
/// use ivy_base::Events;
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

    /// Returns the internal dispatcher for the specified event type.
    pub fn dispatcher<T: Event>(&self) -> Option<&EventDispatcher<T>> {
        self.dispatchers.get(&TypeId::of::<T>()).map(|val| {
            val.downcast_ref::<EventDispatcher<T>>()
                .expect("Failed to downcast")
        })
    }

    /// Returns the internal dispatcher for the specified event type.
    pub fn dispatcher_mut<T: Event>(&mut self) -> &mut EventDispatcher<T> {
        self.dispatchers
            .entry(TypeId::of::<T>())
            .or_insert_with(new_event_dispatcher::<T>)
            .downcast_mut::<EventDispatcher<T>>()
            .expect("Failed to downcast")
    }

    /// Sends an event of type `T` to all subscribed listeners.
    /// If no dispatcher exists for event `T`, a new one will be created.
    pub fn send<T: Event>(&self, event: T) {
        if let Some(dispatcher) = self.dispatcher() {
            dispatcher.send(event)
        }
    }

    /// Shorthand to subscribe using a flume channel.
    pub fn subscribe<T: Event>(&mut self) -> flume::Receiver<T> {
        let (tx, rx) = flume::unbounded();

        self.dispatcher_mut().subscribe(tx, |_| true);

        rx
    }

    /// Subscribes to an event of type T by sending events to teh provided
    /// channel
    pub fn subscribe_custom<S, T: Event>(&mut self, sender: S)
    where
        S: 'static + EventSender<T> + Send,
    {
        self.dispatcher_mut().subscribe(sender, |_| true)
    }

    /// Subscribes to an event of type T by sending events to teh provided
    /// channel
    pub fn subscribe_filter<S, T: Event>(&mut self, sender: S, filter: fn(&T) -> bool)
    where
        S: 'static + EventSender<T> + Send,
    {
        self.dispatcher_mut().subscribe(sender, filter)
    }

    /// Blocks all events of a certain type. All events sent will be silently
    /// ignored.
    pub fn block<T: Event>(&mut self, block: bool) {
        self.dispatcher_mut::<T>().blocked = block
    }

    /// Return true if events of type T are blocked
    pub fn is_blocked<T: Event>(&mut self) -> bool {
        self.dispatcher_mut::<T>().blocked
    }
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

// Blanket type for events.
pub trait Event: Component + Clone {}
impl<T: Component + Clone> Event for T {}

trait AnyEventDispatcher: 'static + Send + Sync + Downcast {}
impl_downcast!(AnyEventDispatcher);

/// Handles event dispatching for a single type of event
pub struct EventDispatcher<T: Event> {
    subscribers: Mutex<Vec<Subscriber<T>>>,
    blocked: bool,
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

impl<T> Subscriber<T> {
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

fn new_event_dispatcher<T: Event>() -> Box<dyn AnyEventDispatcher> {
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
        events.subscribe_custom(tx1);

        let (tx2, rx2) = mpsc::channel::<&'static str>();
        events.subscribe_custom(tx2);

        events.send("Hello");

        if let Ok(e) = rx1.try_recv() {
            assert_eq!(e, "Hello")
        }

        if let Ok(e) = rx2.try_recv() {
            assert_eq!(e, "Hello")
        }
    }
}
