# Ivy


## What it is

Ivy is a modular application and game framework for Rust.

This crate provides an amalgamation and rexport as well as aliasing for the
multiple `ivy` crates.

## [Guide](https://ten3roberts.github.io/ivy)

A user guide is provided to quickly familiarize the user with the basic usage of
the engine.

## Features
  - Vulkan high level rendering
  - PBR rendering and post processing
  - Rendergraph abstractions
  - Collision detection and realistic physics response
  - ECS driven architecture
  - Deferred dynamic events using observer pattern
  - Ray casting for arbitrary convex shapes
  - Handle based dynamic storage
  - Input system with composeable vector generation
  - UI system with configurable widget and positioning system
  - ... And more

## How it works

### Layers

The core of the program is an application. [`core::App`]. It defines the
update loop, and event handling.

From there, logic is extracted into layers which are run for each iteration.
Within a layer, the user is free to do whatever they want, from reading from
sockets, rendering using vulkan, or dispatching ECS workloads.

Due to the layered design, several high level concepts can work together and
not interfere, aswell as being inserted based on different configurations.

Layers can be thought of as plugin in high level containers of behaviour.

The existance of layer allow importing of behaviour from other crates without
concern of implementation details.

### Inter-layer communication
The application exposes different ways in which two layers can influence
each other.

- `world` contains the ECS world with all entities and components.
- `resources` is a typed storage accessed by handles. This is useful for
storing textures, models, or singletons that are to be shared between layers
and inside layers with dynamic borrow checking.
- `events` facilitates a broadcasting channel in which events can be sent
and listened to. Each layer can set up a receiver and iterate the sent events
of a specific type. This is best used for low frequency data to avoid busy
checking, like user input, state changes, or alike.

See the documentation for [`core::Layer`]
