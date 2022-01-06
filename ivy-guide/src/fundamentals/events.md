# Events
To facilitate intra and interlayer communication a deferred bus like event
system is provided.

Any user can subscribe to an event by using a channel. Every layer of type `T`
will be broadcaasted to every subscribed sender. The event can thereafter be
read by iterating the receiving half.

By default, [std::mpsc::channel](https://doc.rust-lang.org/std/sync/mpsc/index.html), [flume](https://docs.rs/flume), and [crossbeam-channel](https://docs.rs/crossbeam-channel/0.5.1/crossbeam_channel/) *(feature = "crossbeam-channel")* implement the `EventSender` trait.

The events will not be cleared between different frames but rather consumed when
iterated. This allows layers which execute occasionally to not miss any events.

Any `'static` `Send` + `Sync` + `Clone` type can be used as an event. However, if
cloning is expensive, consider wrapping the event in an `Arc` or referring to it
by other means as the event will be cloned for every subscribed sender.

## Example
```rust
{{ #include ../../../tests/events.rs:4:27 }}
```
## Intercepting
Sometimes it is necessary to intercept events, either absorbing them or
re-emitting them. This can be accomplished in two main ways.

Events of a certain type can be sent and consumed, to then be resent using a
different type. The final consumers should then subscribe to the latter type.

Sometimes however, it is not possible to re-emit events; either because of
already existing architecture, or that the intercepting component may not always
be present and thus requiring a *mockup* intercepter that simply re-emits.

For these use cases, the use of the `intercept` API is necessary.

```rust
{{ #include ../../../tests/events.rs:32:54 }}
```
