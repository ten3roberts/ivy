//! Ivy-base
//!
//! The base of the ivy framework. This crate provides some of the most used
//! types and traits which many of the other crates depend on. Check [crate::components] for more information.
//!
//! ## App
//! The [App](crate::App) is the heart of any Ivy program. It defines the broad
//! behaviour by layers. Each layer is a set of logic which can be exectuted
//! with minimal shared data from other layers.

//! A common pattern is to have the
//! graphics as one layer, and the game logic as another. This ensures that the
//! code is kept simple such that the game does not need to be concerned with
//! rendering the world, and the rendering does not need to be concerned with
//! the game logic.
//!
//! The layered design allows for easily customizable games as behaviors can be
//! added conditionally, for example a network layer or similar.
//!
//! ## Gizmo
//! The crate also exports a gizmos system [crate::gizmos] which allows the
//! creation of temporary "objects" that can be renderered into the world to
//! provide debuggable feedback.
//!
//! **Note**: The crate is not responsible for rendering the gizmos, but rather
//! provides an agnostic backend for gizmo management. Most commonly,
//! [ivy-graphics::gizmos] is used for rendering the gizmos, but is not
//! required. Gizmos could just as well be rendered in text or an Ncurses like
//! interface.

mod app;
mod color;
pub mod components;
mod dir;
mod events;
pub mod extensions;
mod extent;
pub mod gizmos;
mod layer;
mod logger;
pub mod macros;
mod time;
mod updatable;

use std::f32::consts::PI;

pub use app::{App, AppBuilder, AppEvent};
pub use color::*;
pub use components::*;
pub use dir::*;
pub use events::{EventSender, Events};
pub use extensions::*;
pub use extent::*;
pub use gizmos::*;
pub use layer::*;
pub use logger::Logger;
pub use time::{Clock, FromDuration, IntoDuration, TimedScope};
pub use updatable::*;

/// 45 degrees in radians
pub const DEG_45: f32 = PI * 0.25;
/// 90 degrees in radians
pub const DEG_90: f32 = PI * 0.5;
/// 180 degrees in radians
pub const DEG_180: f32 = PI;
