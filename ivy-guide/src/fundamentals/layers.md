# Layers

The core of the program is an application, which defines the update loop.

The update loop dispatches to several layers, which which are *mostly* self
contained units of logic. The layers get exlusive access to the world,
resources, and events and may execute any logic in `on_update`. They also get
exclusive access to `self` which enables them to both read and modify their own
state.

This is useful for games where the main game logic can be contained in one or
more layers and store the state without interfering with other layers such as
physics.

The layered design allows several high level concepts to work together in unison
and separately and allows for logic to be added or removed.

An example would be a game which makes use of a client and server. The binaries
can share most of the code, and the client and server can be separated into
separate layers which allows the client to use all the same game logic as the
server, and vice versa. The server and client layers can also be present at the
same time which allows a self hosted client.

The `on_update` function takes three parameters:
- [World](./ecs.md)
- [Resources](./resources.md)
- [Events](./events.md)

The return type is of `anyhow::Result` and allows for any error to be
propogated.

For a layer to be used it needs to be pushed into the App.

## Example Usage
The layer is a trait which must define an `on_update` function

The following examples shows the basic usage of a layer, as well how to create
an application using the layer.


```rust
{{#include ../../../examples/layer.rs}}
```
