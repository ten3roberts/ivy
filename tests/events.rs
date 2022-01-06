use ivy::base::Events;
#[test]
fn events() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MyEvent(String);
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OtherEvent(String);

    // Normally, events will be supplied by App
    let mut events = Events::new();
    let my_events = events.subscribe::<MyEvent>();

    let other_events = events.subscribe::<OtherEvent>();

    // Send some events
    events.send(MyEvent(String::from("Hello, World!")));

    // Receive events
    for event in my_events.try_iter() {
        let other = OtherEvent(event.0.to_uppercase());
        events.send(other)
    }

    assert!(other_events
        .try_iter()
        .map(|val| val.0)
        .eq(["HELLO, WORLD!"]));
}

#[test]
fn intercept_events() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MyEvent(String);

    // Normally, events will be supplied by App
    let mut events = Events::new();
    let (tx, my_events) = flume::unbounded();
    events.intercept::<MyEvent, _>(tx).unwrap();

    let other_events = events.subscribe::<MyEvent>();

    // Send some events
    events.send(MyEvent(String::from("Hello, World!")));

    // Receive events
    for event in my_events.try_iter() {
        let other = MyEvent(event.0.to_uppercase());
        events.intercepted_send(other)
    }

    assert!(other_events
        .try_iter()
        .map(|val| val.0)
        .eq(["HELLO, WORLD!"]));
}
