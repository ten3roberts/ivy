//! # Ivy
//!
//! ## What it is
//!
//! Ivy is a modular application and game framework for Rust.
//!
//! This crate exports all ivy crates, but the separate crates can just as well be used manually.
//!
//! ## How it works
//!
//! ### Layers
//! The core of the program is an application. [`core::App`]. It defines the
//! update loop, and event handling.
//!
//! From there, logic is extracted into layers which are run for each iteration.
//! Within a layer, the user is free to do whatever they want, from reading from
//! sockets, rendering using vulkan, or dispatching ECS workloads.
//!
//! Due to the layered design, several high level concepts can work together and
//! not interfere, aswell as being inserted based on different configurations.
//!
//! ### Inter-layer communication
//! The application exposes different ways in which two layers can influence
//! each other.
//!
//! - `world` contains the ECS world with all entities and components.
//! - `resources` is a typed storage accessed by handles. This is useful for
//! storing textures, models, or singletons that are to be shared between layers
//! and inside layers with dynamic borrow checking.
//! - `events` facilitates a broadcasting channel in which events can be sent
//! and listened to. Each layer can set up a receiver and iterate the sent events
//! of a specific type. This is best used for low frequency data to avoid busy
//! checking, like user input, state changes, or alike.
//!
//! See the documentation for [`core::Layer`]

/// Rexports
pub use ivy_core as core;
pub use ivy_graphics as graphics;
pub use ivy_image as image;
pub use ivy_input as input;
pub use ivy_physics as physics;
pub use ivy_postprocessing as postprocessing;
pub use ivy_random as random;
pub use ivy_rendergraph as rendergraph;
pub use ivy_resources as resources;
pub use ivy_ui as ui;
pub use ivy_vulkan as vulkan;