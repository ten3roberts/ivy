mod dispatcher;
pub use dispatcher::{EventSender, MpscSender};

use std::{
    any::{type_name, TypeId},
    collections::HashMap,
    error::Error,
    fmt::Display,
};

use hecs::Component;

use self::dispatcher::{
    new_event_dispatcher, AnyEventDispatcher, AnyEventSender, ConcreteSender, EventDispatcher,
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct AlreadyIntercepted {
    ty: &'static str,
}

impl Display for AlreadyIntercepted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Events of type {:?} have already been intercepted",
            self.ty
        )
    }
}

impl Error for AlreadyIntercepted {}

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
    // A single receiver to intercept events
    intercepts: HashMap<TypeId, Box<dyn AnyEventSender>>,
}

impl Events {
    pub fn new() -> Events {
        Self {
            dispatchers: HashMap::new(),
            intercepts: HashMap::new(),
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
        if let Some(intercept) = self.intercepts.get(&TypeId::of::<T>()) {
            intercept
                .downcast_ref::<ConcreteSender<T>>()
                .unwrap()
                .send(event);
        } else if let Some(dispatcher) = self.dispatcher() {
            dispatcher.send(event)
        }
    }

    /// Send an event after intercept, this function avoids intercepts.
    /// It can also be useful if the message is not supposed to be intercepted
    pub fn intercepted_send<T: Event>(&self, event: T) {
        if let Some(dispatcher) = self.dispatcher() {
            dispatcher.send(event)
        }
    }

    /// Intercept an event before it is broadcasted. Use
    /// `Events::intercepted_send` to send.
    pub fn intercept<T: Event, S: EventSender<T>>(
        &mut self,
        sender: S,
    ) -> Result<(), AlreadyIntercepted> {
        match self.intercepts.entry(TypeId::of::<T>()) {
            std::collections::hash_map::Entry::Occupied(_) => Err(AlreadyIntercepted {
                ty: type_name::<T>(),
            }),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Box::new(ConcreteSender::new(sender)));
                Ok(())
            }
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
        S: EventSender<T>,
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

    /// Remove disconnected subscribers
    pub fn cleanup(&mut self) {
        for (_, dispatcher) in self.dispatchers.iter_mut() {
            dispatcher.cleanup()
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_broadcast() {
        let mut events = Events::new();

        let (tx1, rx1) = flume::unbounded::<&'static str>();
        events.subscribe_custom(tx1);

        let (tx2, rx2) = flume::unbounded::<&'static str>();
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
